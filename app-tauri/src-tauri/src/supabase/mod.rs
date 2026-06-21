#![allow(dead_code)]
/// Supabase REST client stub — will be replaced with real reqwest-based client
/// Matches electron/config/supabase.cjs

pub struct SupabaseClient;

impl SupabaseClient {
    pub fn new() -> Self {
        Self
    }

    pub fn is_configured(&self) -> bool {
        // TODO: check env vars
        false
    }
}

pub fn get_client() -> Option<SupabaseClient> {
    // TODO: implement real client with reqwest
    None
}
