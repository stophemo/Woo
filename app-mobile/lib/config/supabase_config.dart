/// Supabase 配置（与桌面端共用同一个项目）
///
/// anon key 设计上就是公开的，数据安全由 RLS 策略保障。
/// 详见：https://supabase.com/docs/guides/auth/row-level-security
class SupabaseConfig {
  SupabaseConfig._();

  static const String url = 'https://urgpunxxupufmygmutxa.supabase.co';
  static const String anonKey =
      'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InVyZ3B1bnh4dXB1Zm15Z211dHhhIiwicm9sZSI6ImFub24iLCJpYXQiOjE3Nzk1MTA1OTYsImV4cCI6MjA5NTA4NjU5Nn0.Ezs2uFwgrxkwCpLVfdByFC8m5PNjjP92i26Sg0YX-RI';
}
