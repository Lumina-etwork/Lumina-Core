//! Revenue Prediction Engine
//! 
//! This module implements algorithms for predicting creator earnings based on:
//! - Active revenue streams
//! - Historical churn rates
//! - Growth trends
//! - Seasonal patterns

use chrono::{DateTime, Duration, Utc};
use ndarray::{Array1, Array2};
use rand::distributions::Distribution;
use statrs::distribution::{Normal, StudentsT};
use serde::{Deserialize, Serialize};

/// Revenue prediction data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenuePrediction {
    pub period_days: u32,
    pub predicted_revenue: f64,
    pub confidence_interval: ConfidenceInterval,
    pub factors: PredictionFactors,
}

/// Confidence interval for predictions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceInterval {
    pub lower_bound: f64,
    pub upper_bound: f64,
    pub confidence_level: f64, // e.g., 0.95 for 95% CI
}

/// Factors influencing the prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionFactors {
    pub base_revenue: f64,
    pub churn_rate: f64,
    pub growth_rate: f64,
    pub volatility: f64,
    pub stream_count: i32,
}

/// Historical data point for analysis
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct HistoricalStreamData {
    pub timestamp: DateTime<Utc>,
    pub revenue: f64,
    pub active_streams: i32,
    pub cancellations: i32,
}

/// Main prediction engine
pub struct RevenuePredictor {
    /// Minimum data points required for reliable prediction
    min_data_points: usize,
    /// Default prediction periods (30, 60, 90 days)
    prediction_periods: Vec<u32>,
}

impl RevenuePredictor {
    pub fn new() -> Self {
        Self {
            min_data_points: 10,
            prediction_periods: vec![30, 60, 90],
        }
    }

    /// Calculate churn rate from historical data
    pub fn calculate_churn_rate(&self, data: &[HistoricalStreamData]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }

        let total_streams: i32 = data.iter().map(|d| d.active_streams).sum();
        let total_cancellations: i32 = data.iter().map(|d| d.cancellations).sum();

        if total_streams == 0 {
            return 0.0;
        }

        total_cancellations as f64 / total_streams as f64
    }

    /// Calculate revenue growth rate using linear regression
    pub fn calculate_growth_rate(&self, data: &[HistoricalStreamData]) -> f64 {
        if data.len() < self.min_data_points {
            return 0.0;
        }

        // Prepare data for regression
        let n = data.len();
        let mut x_vals = Array1::zeros(n);
        let mut y_vals = Array1::zeros(n);

        let base_time = data[0].timestamp.timestamp() as f64;
        for (i, point) in data.iter().enumerate() {
            x_vals[i] = (point.timestamp.timestamp() - base_time as i64) as f64 / 86400.0; // Days
            y_vals[i] = point.revenue.ln(); // Log transform for stability
        }

        // Simple linear regression
        let x_mean = x_vals.mean().unwrap_or(0.0);
        let y_mean = y_vals.mean().unwrap_or(0.0);

        let numerator: f64 = x_vals.iter().zip(y_vals.iter())
            .map(|(x, y)| (x - x_mean) * (y - y_mean))
            .sum();

        let denominator: f64 = x_vals.iter()
            .map(|x| (x - x_mean).powi(2))
            .sum();

        if denominator == 0.0 {
            return 0.0;
        }

        let slope = numerator / denominator;
        
        // Convert log slope to daily growth rate
        slope.exp() - 1.0
    }

    /// Calculate revenue volatility (standard deviation of returns)
    pub fn calculate_volatility(&self, data: &[HistoricalStreamData]) -> f64 {
        if data.len() < 2 {
            return 0.0;
        }

        let returns: Vec<f64> = data.windows(2)
            .map(|w| {
                if w[0].revenue == 0.0 {
                    0.0
                } else {
                    (w[1].revenue - w[0].revenue) / w[0].revenue
                }
            })
            .collect();

        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / (returns.len() - 1) as f64;

        variance.sqrt()
    }

    /// Predict revenue for a given period using Monte Carlo simulation
    pub fn predict_revenue(
        &self,
        historical_data: &[HistoricalStreamData],
        period_days: u32,
    ) -> Option<RevenuePrediction> {
        if historical_data.len() < self.min_data_points {
            return None;
        }

        // Calculate key metrics
        let churn_rate = self.calculate_churn_rate(historical_data);
        let growth_rate = self.calculate_growth_rate(historical_data);
        let volatility = self.calculate_volatility(historical_data);
        
        // Use most recent revenue as base
        let base_revenue = historical_data.last()?.revenue;
        let stream_count = historical_data.last()?.active_streams;

        // Monte Carlo simulation (1000 iterations)
        let simulations = 1000;
        let mut rng = rand::thread_rng();
        let mut predicted_revenues = Vec::with_capacity(simulations);

        for _ in 0..simulations {
            let mut revenue = base_revenue;
            let daily_volatility = volatility / (30.0_f64).sqrt(); // Daily vol from monthly

            for _day in 0..period_days {
                // Apply growth and churn
                let net_growth = growth_rate / 30.0 - churn_rate / 30.0;
                revenue *= 1.0 + net_growth;

                // Add random shock (geometric Brownian motion)
                let shock = Normal::new(0.0, daily_volatility)
                    .ok()?
                    .sample(&mut rng);
                revenue *= 1.0 + shock;

                // Ensure non-negative
                revenue = revenue.max(0.0);
            }

            predicted_revenues.push(revenue);
        }

        // Calculate statistics
        predicted_revenues.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let mean_revenue = predicted_revenues.iter().sum::<f64>() / simulations as f64;
        
        // 95% confidence interval
        let lower_idx = (simulations as f64 * 0.025) as usize;
        let upper_idx = (simulations as f64 * 0.975) as usize;

        Some(RevenuePrediction {
            period_days,
            predicted_revenue: mean_revenue,
            confidence_interval: ConfidenceInterval {
                lower_bound: predicted_revenues[lower_idx],
                upper_bound: predicted_revenues[upper_idx],
                confidence_level: 0.95,
            },
            factors: PredictionFactors {
                base_revenue,
                churn_rate,
                growth_rate,
                volatility,
                stream_count,
            },
        })
    }

    /// Generate predictions for standard periods (30, 60, 90 days)
    pub fn generate_all_predictions(
        &self,
        historical_data: &[HistoricalStreamData],
    ) -> Vec<RevenuePrediction> {
        self.prediction_periods
            .iter()
            .filter_map(|&period| self.predict_revenue(historical_data, period))
            .collect()
    }
}

impl Default for RevenuePredictor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_sample_data() -> Vec<HistoricalStreamData> {
        let mut data = Vec::new();
        let mut revenue = 1000.0;
        
        for day in 0..30 {
            data.push(HistoricalStreamData {
                timestamp: Utc::now() - Duration::days((30 - day) as i64),
                revenue,
                active_streams: 10,
                cancellations: (day % 3) as i32,
            });
            
            // Simulate some growth with noise
            revenue *= 1.01 + (day as f64 * 0.001);
        }
        
        data
    }

    #[test]
    fn test_calculate_churn_rate() {
        let predictor = RevenuePredictor::new();
        let data = create_sample_data();
        
        let churn = predictor.calculate_churn_rate(&data);
        assert!(churn > 0.0 && churn < 1.0);
    }

    #[test]
    fn test_predict_revenue() {
        let predictor = RevenuePredictor::new();
        let data = create_sample_data();
        
        let prediction = predictor.predict_revenue(&data, 30);
        assert!(prediction.is_some());
        
        let pred = prediction.unwrap();
        assert!(pred.predicted_revenue > 0.0);
        assert!(pred.confidence_interval.lower_bound <= pred.predicted_revenue);
        assert!(pred.confidence_interval.upper_bound >= pred.predicted_revenue);
    }

    #[test]
    fn test_generate_all_predictions() {
        let predictor = RevenuePredictor::new();
        let data = create_sample_data();
        
        let predictions = predictor.generate_all_predictions(&data);
        assert_eq!(predictions.len(), 3); // 30, 60, 90 days
        
        // Longer periods should have wider confidence intervals
        if predictions.len() >= 2 {
            let range_30 = predictions[0].confidence_interval.upper_bound 
                - predictions[0].confidence_interval.lower_bound;
            let range_90 = predictions[2].confidence_interval.upper_bound 
                - predictions[2].confidence_interval.lower_bound;
            assert!(range_90 > range_30);
        }
    }
}
