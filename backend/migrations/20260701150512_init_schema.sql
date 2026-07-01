
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(100) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS mpc_accounts_pool (
    public_key VARCHAR(130) PRIMARY KEY,
    user_id UUID UNIQUE REFERENCES users(id) ON DELETE SET NULL,
    assigned_at TIMESTAMP WITH TIME ZONE
);

CREATE INDEX IF NOT EXISTS idx_unassigned_mpc ON mpc_accounts_pool (public_key) WHERE user_id IS NULL;

CREATE TABLE IF NOT EXISTS collateral (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    available_balance NUMERIC(20, 8) NOT NULL DEFAULT 0.00000000,
    locked_balance NUMERIC(20, 8) NOT NULL DEFAULT 0.00000000,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE TYPE order_side AS ENUM ('BUY', 'SELL');
CREATE TYPE order_type AS ENUM ('LIMIT', 'MARKET');
CREATE TYPE order_status AS ENUM ('OPEN', 'FILLED', 'CANCELLED', 'REJECTED');

CREATE TABLE IF NOT EXISTS orders (
    order_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    market_id VARCHAR(20) NOT NULL,
    side order_side NOT NULL,
    order_type order_type NOT NULL,
    quantity NUMERIC(20, 8) NOT NULL,
    price NUMERIC(20, 8) NOT NULL,
    margin NUMERIC(20, 8) NOT NULL,
    status order_status NOT NULL DEFAULT 'OPEN',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE TYPE position_side AS ENUM ('LONG', 'SHORT');

CREATE TABLE IF NOT EXISTS positions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    market_id VARCHAR(20) NOT NULL,
    side position_side NOT NULL,
    quantity NUMERIC(20, 8) NOT NULL,
    margin NUMERIC(20, 8) NOT NULL,
    average_entry_price NUMERIC(20, 8) NOT NULL,
    liquidation_price NUMERIC(20, 8) NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(user_id, market_id)
)