import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../config/constants.dart';
import '../services/supabase_service.dart';
import '../providers/providers.dart';

/// 设置页面
class SettingsScreen extends ConsumerWidget {
  const SettingsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final syncStatus = ref.watch(syncStatusProvider);
    final auth = ref.watch(authStateProvider);

    return Scaffold(
      appBar: AppBar(
        title: const Text('设置'),
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          // Account section
          _SectionHeader(title: '账户'),
          Card(
            child: Column(
              children: [
                ListTile(
                  leading: auth.when(
                    data: (loggedIn) => Icon(
                      loggedIn ? Icons.cloud_done : Icons.cloud_off,
                      color: loggedIn ? Colors.green : theme.colorScheme.onSurfaceVariant,
                    ),
                    error: (_, __) => const Icon(Icons.cloud_off),
                    loading: () => const Icon(Icons.cloud_outlined),
                  ),
                  title: auth.when(
                    data: (loggedIn) => Text(loggedIn ? '已登录' : '未登录'),
                    error: (_, __) => const Text('未登录'),
                    loading: () => const Text('加载中…'),
                  ),
                  subtitle: auth.when(
                    data: (loggedIn) {
                      if (!loggedIn) return const Text('登录后可同步到云端');
                      final user = SupabaseService.currentUser;
                      return Text(user?.email ?? '');
                    },
                    error: (_, __) => const Text('连接失败'),
                    loading: () => const Text(''),
                  ),
                  trailing: auth.when(
                    data: (loggedIn) {
                      if (loggedIn) {
                        return TextButton(
                          onPressed: () async {
                            await SupabaseService.signOut();
                          },
                          child: const Text('退出'),
                        );
                      }
                      return TextButton(
                        onPressed: () => _showLoginDialog(context, ref),
                        child: const Text('登录'),
                      );
                    },
                    error: (_, __) => null,
                    loading: () => null,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 24),

          // Sync section
          _SectionHeader(title: '同步'),
          Card(
            child: Column(
              children: [
                ListTile(
                  leading: Icon(
                    syncStatus.isSyncing ? Icons.sync : Icons.sync_disabled,
                    color: syncStatus.isSyncing
                        ? theme.colorScheme.primary
                        : theme.colorScheme.onSurfaceVariant,
                  ),
                  title: Text(syncStatus.isSyncing ? '同步中…' : '同步状态'),
                  subtitle: Text(syncStatus.lastSyncTime != null
                      ? '上次同步: ${_formatDate(syncStatus.lastSyncTime!)}'
                      : '尚未同步'),
                ),
                ListTile(
                  leading: const Icon(Icons.timer_outlined),
                  title: const Text('同步间隔'),
                  subtitle: const Text('60 秒（自动）'),
                ),
              ],
            ),
          ),
          const SizedBox(height: 24),

          // Storage section
          _SectionHeader(title: '存储'),
          Card(
            child: ListTile(
              leading: const Icon(Icons.storage_outlined),
              title: const Text('本地数据库'),
              subtitle: const Text(AppConstants.dbName),
            ),
          ),
          const SizedBox(height: 24),

          // About section
          _SectionHeader(title: '关于'),
          Card(
            child: Column(
              children: [
                ListTile(
                  leading: const Icon(Icons.info_outline),
                  title: const Text('应用版本'),
                  subtitle: Text(AppConstants.appVersion),
                ),
                ListTile(
                  leading: const Icon(Icons.code),
                  title: const Text('无我笔记 (Woo)'),
                  subtitle: const Text('本地优先的 Markdown 笔记'),
                ),
              ],
            ),
          ),
          const SizedBox(height: 32),

          // Sync now button
          Center(
            child: OutlinedButton.icon(
              onPressed: () {
                ref.read(syncEngineProvider).syncNow();
              },
              icon: const Icon(Icons.sync),
              label: const Text('立即同步'),
            ),
          ),
          const SizedBox(height: 32),
        ],
      ),
    );
  }
}

class _SectionHeader extends StatelessWidget {
  final String title;
  const _SectionHeader({required this.title});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(left: 4, bottom: 8),
      child: Text(
        title,
        style: Theme.of(context).textTheme.titleSmall?.copyWith(
              color: Theme.of(context).colorScheme.primary,
              fontWeight: FontWeight.w600,
            ),
      ),
    );
  }
}

Future<void> _showLoginDialog(BuildContext context, WidgetRef ref) async {
  final emailCtrl = TextEditingController();
  final passwordCtrl = TextEditingController();

  await showDialog(
    context: context,
    builder: (ctx) => AlertDialog(
      title: const Text('登录'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          TextField(
            controller: emailCtrl,
            decoration: const InputDecoration(
              labelText: '邮箱',
              border: OutlineInputBorder(),
            ),
            keyboardType: TextInputType.emailAddress,
          ),
          const SizedBox(height: 12),
          TextField(
            controller: passwordCtrl,
            decoration: const InputDecoration(
              labelText: '密码',
              border: OutlineInputBorder(),
            ),
            obscureText: true,
          ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(),
          child: const Text('取消'),
        ),
        FilledButton(
          onPressed: () async {
            try {
              await SupabaseService.signInWithEmail(
                emailCtrl.text.trim(),
                passwordCtrl.text,
              );
              if (ctx.mounted) Navigator.of(ctx).pop();
            } catch (e) {
              if (ctx.mounted) {
                ScaffoldMessenger.of(ctx).showSnackBar(
                  SnackBar(content: Text('登录失败: $e')),
                );
              }
            }
          },
          child: const Text('登录'),
        ),
      ],
    ),
  );
}

String _formatDate(String iso) {
  try {
    final dt = DateTime.parse(iso).toLocal();
    return '${dt.month}/${dt.day} ${dt.hour.toString().padLeft(2, '0')}:${dt.minute.toString().padLeft(2, '0')}';
  } catch (_) {
    return '';
  }
}
