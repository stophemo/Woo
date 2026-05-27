import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'config/constants.dart';
import 'providers/providers.dart';
import 'screens/home_screen.dart';
import 'screens/auth_screen.dart';
import 'screens/settings_screen.dart';
import 'screens/trash_screen.dart';

class WooApp extends ConsumerWidget {
  const WooApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return MaterialApp(
      title: AppConstants.appName,
      debugShowCheckedModeBanner: false,
      theme: _buildLightTheme(),
      darkTheme: _buildDarkTheme(),
      themeMode: ThemeMode.system,
      home: const _AppRoot(),
    );
  }

  ThemeData _buildLightTheme() {
    final colorScheme = ColorScheme.fromSeed(
      seedColor: const Color(0xFF4A90D9),
      brightness: Brightness.light,
    );
    return ThemeData(
      colorScheme: colorScheme,
      useMaterial3: true,
      appBarTheme: AppBarTheme(
        centerTitle: true,
        backgroundColor: colorScheme.surface,
        foregroundColor: colorScheme.onSurface,
        elevation: 0,
        scrolledUnderElevation: 0.5,
      ),
    );
  }

  ThemeData _buildDarkTheme() {
    final colorScheme = ColorScheme.fromSeed(
      seedColor: const Color(0xFF4A90D9),
      brightness: Brightness.dark,
    );
    return ThemeData(
      colorScheme: colorScheme,
      useMaterial3: true,
      appBarTheme: AppBarTheme(
        centerTitle: true,
        backgroundColor: colorScheme.surface,
        foregroundColor: colorScheme.onSurface,
        elevation: 0,
        scrolledUnderElevation: 0.5,
      ),
    );
  }
}

/// Root — 初始化 DB/Sync 并决定显示认证页还是主应用
class _AppRoot extends ConsumerStatefulWidget {
  const _AppRoot();

  @override
  ConsumerState<_AppRoot> createState() => _AppRootState();
}

class _AppRootState extends ConsumerState<_AppRoot> {
  bool _dbReady = false;
  bool _initFailed = false;

  @override
  void initState() {
    super.initState();
    _init();
  }

  Future<void> _init() async {
    try {
      await ref.read(databaseReadyProvider.future);
      // 后台初始化 Supabase + Sync（出错不阻塞）
      ref.read(syncInitProvider);
      setState(() => _dbReady = true);
    } catch (e) {
      setState(() => _initFailed = true);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_initFailed) {
      return Scaffold(
        body: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(Icons.error_outline, size: 64,
                  color: Theme.of(context).colorScheme.error),
              const SizedBox(height: 16),
              const Text('初始化数据库失败'),
              const SizedBox(height: 16),
              FilledButton.icon(
                onPressed: () {
                  setState(() => _initFailed = false);
                  _init();
                },
                icon: const Icon(Icons.refresh),
                label: const Text('重试'),
              ),
            ],
          ),
        ),
      );
    }

    if (!_dbReady) {
      return Scaffold(
        body: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              CircularProgressIndicator(
                  color: Theme.of(context).colorScheme.primary),
              const SizedBox(height: 16),
              const Text('正在加载…'),
            ],
          ),
        ),
      );
    }

    // 判断是否显示认证页 — 未登录 + 非本地模式 → AuthScreen
    return Consumer(
      builder: (context, ref, _) {
        final isLocalMode = ref.watch(localModeProvider);
        final authAsync = ref.watch(authStateProvider);

        // 本地模式跳过登录
        if (isLocalMode) return const AppShell();

        return authAsync.when(
          data: (loggedIn) => loggedIn ? const AppShell() : const AuthScreen(),
          error: (_, __) => const AppShell(), // 网络异常 → 默认进本地模式
          loading: () => const Scaffold(
            body: Center(child: CircularProgressIndicator()),
          ),
        );
      },
    );
  }
}

/// 主应用壳 — 底部 3 tab 导航
class AppShell extends ConsumerStatefulWidget {
  const AppShell({super.key});

  @override
  ConsumerState<AppShell> createState() => _AppShellState();
}

class _AppShellState extends ConsumerState<AppShell> {
  int _currentIndex = 0;

  final _screens = const [
    HomeScreen(),
    TrashScreen(),
    SettingsScreen(),
  ];

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: IndexedStack(
        index: _currentIndex,
        children: _screens,
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: _currentIndex,
        onDestinationSelected: (i) => setState(() => _currentIndex = i),
        destinations: const [
          NavigationDestination(
            icon: Icon(Icons.note_outlined),
            selectedIcon: Icon(Icons.note),
            label: '笔记',
          ),
          NavigationDestination(
            icon: Icon(Icons.delete_outlined),
            selectedIcon: Icon(Icons.delete),
            label: '回收站',
          ),
          NavigationDestination(
            icon: Icon(Icons.settings_outlined),
            selectedIcon: Icon(Icons.settings),
            label: '设置',
          ),
        ],
      ),
    );
  }
}
