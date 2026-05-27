import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../services/supabase_service.dart';
import '../providers/providers.dart';

/// 登录/注册页面
///
/// Supabase 免费层支持邮箱密码认证，与桌面端共用同一套用户系统。
/// 也支持「仅本地使用」跳过登录。
class AuthScreen extends ConsumerWidget {
  const AuthScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return const _AuthForm();
  }
}

class _AuthForm extends ConsumerStatefulWidget {
  const _AuthForm();

  @override
  ConsumerState<_AuthForm> createState() => _AuthFormState();
}

class _AuthFormState extends ConsumerState<_AuthForm> {
  bool _isLogin = true;
  bool _loading = false;
  String? _error;

  final _emailCtrl = TextEditingController();
  final _passwordCtrl = TextEditingController();
  final _usernameCtrl = TextEditingController();

  @override
  void dispose() {
    _emailCtrl.dispose();
    _passwordCtrl.dispose();
    _usernameCtrl.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    setState(() {
      _loading = true;
      _error = null;
    });

    try {
      if (_isLogin) {
        await SupabaseService.signInWithEmail(
          _emailCtrl.text.trim(),
          _passwordCtrl.text,
        );
        // authStateProvider 的 stream 会自动通知 app.dart 切换到 _AppShell
      } else {
        final username = _usernameCtrl.text.trim();
        if (username.isEmpty) throw Exception('用户名不能为空');
        await SupabaseService.signUp(
          _emailCtrl.text.trim(),
          _passwordCtrl.text,
          username,
        );
        if (!mounted) return;
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('注册成功，请查看邮箱确认链接')),
        );
        setState(() => _isLogin = true);
      }
    } catch (e) {
      setState(() => _error = e.toString().replaceFirst('Exception: ', ''));
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  void _skipToLocalMode() {
    // 设置本地模式 → app.dart 的 _AppRoot 会显示 _AppShell
    ref.read(localModeProvider.notifier).state = true;
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Scaffold(
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(32),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 400),
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  // Logo
                  Container(
                    width: 80,
                    height: 80,
                    decoration: BoxDecoration(
                      color: theme.colorScheme.primaryContainer,
                      borderRadius: BorderRadius.circular(20),
                    ),
                    child: Icon(
                      Icons.auto_stories,
                      size: 40,
                      color: theme.colorScheme.onPrimaryContainer,
                    ),
                  ),
                  const SizedBox(height: 16),
                  Text(
                    '无我笔记',
                    style: theme.textTheme.headlineMedium
                        ?.copyWith(fontWeight: FontWeight.bold),
                  ),
                  const SizedBox(height: 8),
                  Text(
                    _isLogin ? '登录以跨设备同步' : '创建新账户',
                    style: theme.textTheme.bodyLarge
                        ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                  ),
                  const SizedBox(height: 40),

                  // 注册时需要用户名
                  if (!_isLogin)
                    Padding(
                      padding: const EdgeInsets.only(bottom: 16),
                      child: TextField(
                        controller: _usernameCtrl,
                        decoration: const InputDecoration(
                          labelText: '用户名',
                          prefixIcon: Icon(Icons.person_outline),
                          border: OutlineInputBorder(),
                        ),
                        textInputAction: TextInputAction.next,
                      ),
                    ),
                  Padding(
                    padding: const EdgeInsets.only(bottom: 16),
                    child: TextField(
                      controller: _emailCtrl,
                      decoration: const InputDecoration(
                        labelText: '邮箱',
                        prefixIcon: Icon(Icons.email_outlined),
                        border: OutlineInputBorder(),
                      ),
                      keyboardType: TextInputType.emailAddress,
                      textInputAction: TextInputAction.next,
                    ),
                  ),
                  Padding(
                    padding: const EdgeInsets.only(bottom: 16),
                    child: TextField(
                      controller: _passwordCtrl,
                      decoration: const InputDecoration(
                        labelText: '密码',
                        prefixIcon: Icon(Icons.lock_outlined),
                        border: OutlineInputBorder(),
                      ),
                      obscureText: true,
                      textInputAction: TextInputAction.done,
                      onSubmitted: (_) => _submit(),
                    ),
                  ),

                  if (_error != null)
                    Padding(
                      padding: const EdgeInsets.only(bottom: 16),
                      child: Text(
                        _error!,
                        style: TextStyle(color: theme.colorScheme.error),
                      ),
                    ),

                  FilledButton(
                    onPressed: _loading ? null : _submit,
                    style: FilledButton.styleFrom(
                      minimumSize: const Size(double.infinity, 48),
                    ),
                    child: _loading
                        ? const SizedBox(
                            width: 20,
                            height: 20,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : Text(_isLogin ? '登录' : '注册'),
                  ),
                  const SizedBox(height: 16),

                  TextButton(
                    onPressed: () => setState(() => _isLogin = !_isLogin),
                    child: Text(
                        _isLogin ? '没有账户？去注册' : '已有账户？去登录'),
                  ),

                  const SizedBox(height: 8),
                  TextButton.icon(
                    onPressed: _skipToLocalMode,
                    icon: const Icon(Icons.offline_bolt_outlined),
                    label: const Text('跳过登录，仅本地使用'),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
