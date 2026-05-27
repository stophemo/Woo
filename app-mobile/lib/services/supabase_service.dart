import 'package:supabase_flutter/supabase_flutter.dart';
import '../config/supabase_config.dart';

/// Supabase 客户端封装 — 与桌面端共用同一项目
///
/// 移动端和桌面端共用同一套 Supabase 项目，表结构和 RLS 策略。
/// anon key 公开，数据安全由 RLS 行级策略保障。
class SupabaseService {
  static SupabaseClient? _client;

  /// 初始化（App 启动时调用一次）
  static Future<void> initialize() async {
    await Supabase.initialize(
      url: SupabaseConfig.url,
      anonKey: SupabaseConfig.anonKey,
    );
    _client = Supabase.instance.client;
  }

  /// 获取 Supabase 客户端
  static SupabaseClient? get client => _client;

  /// 当前用户
  static User? get currentUser => _client?.auth.currentUser;

  /// 是否已登录
  static bool get isLoggedIn => currentUser != null;

  /// 登录
  static Future<AuthResponse> signInWithEmail(String email, String password) async {
    return await _client!.auth.signInWithPassword(email: email, password: password);
  }

  /// 注册
  static Future<AuthResponse> signUp(String email, String password, String username) async {
    return await _client!.auth.signUp(
      email: email,
      password: password,
      data: {'username': username},
    );
  }

  /// 登出
  static Future<void> signOut() async {
    await _client!.auth.signOut();
  }

  /// 监听认证状态
  static Stream<AuthState> get authStateChanges => _client!.auth.onAuthStateChange;

  /// 获取当前 session
  static Session? get currentSession => _client?.auth.currentSession;
}
