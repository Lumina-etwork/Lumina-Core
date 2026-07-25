-- Database schema for comment system and messaging

-- Users table (extends existing creator/fan base)
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(100) UNIQUE NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    public_key TEXT, -- For E2E encryption
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Creators table
CREATE TABLE IF NOT EXISTS creators (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    display_name VARCHAR(255) NOT NULL,
    bio TEXT,
    avatar_url TEXT,
    is_verified BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Fans table
CREATE TABLE IF NOT EXISTS fans (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    display_name VARCHAR(255) NOT NULL,
    avatar_url TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Subscription tiers
CREATE TABLE IF NOT EXISTS subscription_tiers (
    id SERIAL PRIMARY KEY,
    creator_id UUID NOT NULL REFERENCES creators(user_id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    monthly_price DECIMAL(10, 2) NOT NULL,
    tier_level INTEGER NOT NULL DEFAULT 1, -- 1=Bronze, 2=Silver, 3=Gold, etc.
    permissions JSONB DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Active subscriptions (fans with active streams)
CREATE TABLE IF NOT EXISTS subscriptions (
    id SERIAL PRIMARY KEY,
    fan_id UUID NOT NULL REFERENCES fans(user_id) ON DELETE CASCADE,
    creator_id UUID NOT NULL REFERENCES creators(user_id) ON DELETE CASCADE,
    tier_id INTEGER NOT NULL REFERENCES subscription_tiers(id),
    status VARCHAR(50) NOT NULL DEFAULT 'active', -- active, cancelled, expired
    started_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    ends_at TIMESTAMP WITH TIME ZONE,
    cancelled_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(fan_id, creator_id)
);

-- Indexes for subscription queries
CREATE INDEX idx_subscriptions_fan_creator ON subscriptions(fan_id, creator_id);
CREATE INDEX idx_subscriptions_status ON subscriptions(status);

-- Comments table (threaded)
CREATE TABLE IF NOT EXISTS comments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    creator_id UUID NOT NULL REFERENCES creators(user_id) ON DELETE CASCADE,
    fan_id UUID NOT NULL REFERENCES fans(user_id) ON DELETE CASCADE,
    parent_comment_id UUID REFERENCES comments(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    is_edited BOOLEAN DEFAULT FALSE,
    is_deleted BOOLEAN DEFAULT FALSE,
    like_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    
    -- Ensure fan has active subscription
    CONSTRAINT check_active_subscription CHECK (
        EXISTS (
            SELECT 1 FROM subscriptions s 
            WHERE s.fan_id = fans.user_id 
              AND s.creator_id = comments.creator_id 
              AND s.status = 'active'
        )
    )
);

-- Indexes for comment queries
CREATE INDEX idx_comments_creator ON comments(creator_id);
CREATE INDEX idx_comments_parent ON comments(parent_comment_id);
CREATE INDEX idx_comments_fan ON comments(fan_id);
CREATE INDEX idx_comments_created ON comments(created_at DESC);

-- Comment likes
CREATE TABLE IF NOT EXISTS comment_likes (
    id SERIAL PRIMARY KEY,
    comment_id UUID NOT NULL REFERENCES comments(id) ON DELETE CASCADE,
    fan_id UUID NOT NULL REFERENCES fans(user_id) ON DELETE CASCADE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(comment_id, fan_id)
);

-- Messages table (E2E encrypted)
CREATE TABLE IF NOT EXISTS messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sender_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    recipient_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    encrypted_content TEXT NOT NULL, -- E2E encrypted payload
    nonce TEXT NOT NULL, -- Encryption nonce
    is_read BOOLEAN DEFAULT FALSE,
    read_at TIMESTAMP WITH TIME ZONE,
    sent_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_by_sender BOOLEAN DEFAULT FALSE,
    deleted_by_recipient BOOLEAN DEFAULT FALSE
);

-- Indexes for message queries
CREATE INDEX idx_messages_sender ON messages(sender_id);
CREATE INDEX idx_messages_recipient ON messages(recipient_id);
CREATE INDEX idx_messages_unread ON messages(recipient_id, is_read) WHERE is_read = FALSE;

-- Conversation metadata (for quick lookup)
CREATE TABLE IF NOT EXISTS conversations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    participant_1 UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    participant_2 UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    last_message_at TIMESTAMP WITH TIME ZONE,
    last_message_preview TEXT,
    unread_count INTEGER DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    UNIQUE(participant_1, participant_2)
);

-- Access logs for security auditing
CREATE TABLE IF NOT EXISTS access_logs (
    id SERIAL PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    action VARCHAR(100) NOT NULL,
    resource_type VARCHAR(50),
    resource_id UUID,
    ip_address INET,
    user_agent TEXT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_access_logs_user ON access_logs(user_id);
CREATE INDEX idx_access_logs_action ON access_logs(action);

-- Function to update timestamps
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Apply timestamp triggers
CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_comments_updated_at BEFORE UPDATE ON comments
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- View: Fan eligibility for commenting (has active subscription)
CREATE OR REPLACE VIEW eligible_commenters AS
SELECT 
    f.user_id as fan_id,
    f.display_name,
    s.creator_id,
    s.tier_id,
    st.name as tier_name,
    st.tier_level,
    s.started_at,
    s.ends_at
FROM fans f
JOIN subscriptions s ON f.user_id = s.fan_id
JOIN subscription_tiers st ON s.tier_id = st.id
WHERE s.status = 'active';

-- View: Messaging permissions (Gold tier only example)
CREATE OR REPLACE VIEW messaging_permissions AS
SELECT 
    st.creator_id,
    st.tier_level,
    CASE WHEN st.tier_level >= 3 THEN true ELSE false END as can_dm_creator
FROM subscription_tiers st;

-- Sample data (for testing)
/*
INSERT INTO users (id, username, email, password_hash) VALUES
(gen_random_uuid(), 'testfan', 'fan@example.com', '$argon2...'),
(gen_random_uuid(), 'testcreator', 'creator@example.com', '$argon2...');

INSERT INTO creators (user_id, display_name) 
SELECT id, username FROM users WHERE username = 'testcreator';

INSERT INTO fans (user_id, display_name) 
SELECT id, username FROM users WHERE username = 'testfan';
*/

-- Dead Letter Queue for failed message processing.
-- Payloads are represented by a digest instead of plaintext to preserve E2E
-- confidentiality while still enabling replay correlation and incident triage.
CREATE TABLE IF NOT EXISTS message_dead_letters (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    message_id UUID REFERENCES messages(id) ON DELETE SET NULL,
    sender_id UUID REFERENCES users(id) ON DELETE SET NULL,
    recipient_id UUID REFERENCES users(id) ON DELETE SET NULL,
    failure_stage VARCHAR(100) NOT NULL,
    error_class VARCHAR(100) NOT NULL,
    error_message TEXT NOT NULL,
    payload_digest TEXT NOT NULL,
    retry_count INTEGER NOT NULL DEFAULT 0,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    first_failed_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    last_failed_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    next_retry_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMP WITH TIME ZONE,
    CONSTRAINT message_dead_letters_status_check
        CHECK (status IN ('pending', 'retrying', 'resolved'))
);

CREATE INDEX idx_message_dead_letters_status_retry
    ON message_dead_letters(status, next_retry_at)
    WHERE status IN ('pending', 'retrying');
CREATE INDEX idx_message_dead_letters_message ON message_dead_letters(message_id);
CREATE INDEX idx_message_dead_letters_participants ON message_dead_letters(sender_id, recipient_id);
