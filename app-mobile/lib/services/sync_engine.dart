import 'dart:async';
import 'package:supabase_flutter/supabase_flutter.dart';
import '../database/app_database.dart';
import '../config/constants.dart';
import 'supabase_service.dart';

/// 同步引擎 — 与桌面端 syncEngine.cjs 完全兼容
///
/// 同步协议（与桌面端一致）：
///   1. PULL 墓碑：读取云端 sync_tombstone → 本地真删
///   2. PULL 云端变更：读取云端 update_time > last_sync 的记录 → 合并到本地
///   3. PUSH 本地变更：读取本地 update_time > last_sync 的记录 → upsert 到云端
///   4. 超期清理：清理过期 deleted=2 记录
///   5. 更新 last_sync_time + 回收旧墓碑
///
/// 冲突策略：last-write-wins（与桌面端一致）
class SyncEngine {
  bool _isSyncing = false;
  Completer<SyncResult>? _currentSync;
  String? _lastSyncTime;
  Timer? _timer;
  StreamSubscription<AuthState>? _authSub;

  // Callbacks
  void Function(SyncStatus)? onStatusChange;
  void Function()? onDataChange;

  /// 表列表（与桌面端 ENTITIES 一致）
  static const _entities = [
    ('note_folder', 'id'),
    ('note_document', 'id'),
    ('note_document_version', 'id'),
  ];

  // ═══════════════════════════════════════════════
  // Lifecycle
  // ═══════════════════════════════════════════════

  Future<void> start() async {
    _lastSyncTime = await AppDatabase.getSyncMeta('last_sync_time');

    _authSub = SupabaseService.authStateChanges.listen((authState) {
      final session = SupabaseService.currentUser;
      if (authState.event == AuthChangeEvent.signedIn ||
          authState.event == AuthChangeEvent.tokenRefreshed) {
        if (session != null) {
          AppDatabase.setSyncMeta('last_sync_time', '');
          _lastSyncTime = null;
          Future.delayed(const Duration(milliseconds: 500), () => syncNow());
        }
      } else if (authState.event == AuthChangeEvent.signedOut) {
        _lastSyncTime = null;
      }
    });

    _timer = Timer.periodic(
      const Duration(seconds: AppConstants.syncIntervalSeconds),
      (_) => syncNow(),
    );

    if (SupabaseService.isLoggedIn) {
      Future.delayed(const Duration(seconds: 1), () => syncNow());
    }
  }

  Future<void> stop() async {
    _timer?.cancel();
    _timer = null;
    await _authSub?.cancel();
    _authSub = null;
  }

  // ═══════════════════════════════════════════════
  // Main sync
  // ═══════════════════════════════════════════════

  Future<SyncResult> syncNow() async {
    if (_isSyncing && _currentSync != null) {
      return await _currentSync!.future;
    }

    final completer = Completer<SyncResult>();
    _currentSync = completer;
    _isSyncing = true;
    _emitStatus();

    try {
      final session = SupabaseService.currentUser;
      if (session == null) {
        // 提示：即使未登录也不崩溃，只是不执行同步
        throw Exception('未登录');
      }

      final userId = session.id;
      final lastSync = _lastSyncTime;

      final tombstoneResult = await _pullTombstones(userId);
      final pullResult = await _pullChanges(userId, lastSync);
      final pushResult = await _pushChanges(userId, lastSync, pullResult.pulledIds);
      final cleanupResult = await _cleanupExpiredDeletes(userId);

      final now = DateTime.now().toUtc().toIso8601String();
      await AppDatabase.setSyncMeta('last_sync_time', now);
      _lastSyncTime = now;

      final result = SyncResult(
        pushedCount: pushResult,
        pulledCount: pullResult.count,
        conflictCount: pullResult.conflicts,
        cleanupCount: cleanupResult,
        tombstoneCount: tombstoneResult,
        syncTime: now,
      );
      completer.complete(result);
      return result;
    } catch (e) {
      completer.completeError(e);
      rethrow;
    } finally {
      _isSyncing = false;
      _currentSync = null;
      _emitStatus();
      onDataChange?.call();
    }
  }

  // ═══════════════════════════════════════════════
  // Push
  // ═══════════════════════════════════════════════

  Future<int> _pushChanges(String userId, String? lastSync, Map<String, Set<String>> skipIds) async {
    final client = SupabaseService.client;
    if (client == null) return 0;

    int pushed = 0;

    for (final (table, idField) in _entities) {
      try {
        var rows = await AppDatabase.queryChangesSince(table, lastSync ?? '1970-01-01');
        if (rows.isEmpty) continue;

        final skipSet = skipIds[table];
        if (skipSet != null && skipSet.isNotEmpty) {
          rows = rows.where((r) => !skipSet.contains(r[idField] as String)).toList();
          if (rows.isEmpty) continue;
        }

        final rowsWithUser = rows.map((r) => {...r, 'user_id': userId}).toList();

        await client.from(table).upsert(rowsWithUser, onConflict: 'id', ignoreDuplicates: false);
        pushed += rows.length;
      } catch (_) {
        // 单表推送失败，继续下一张
      }
    }

    return pushed;
  }

  // ═══════════════════════════════════════════════
  // Pull
  // ═══════════════════════════════════════════════

  Future<_PullResult> _pullChanges(String userId, String? lastSync) async {
    final client = SupabaseService.client;
    if (client == null) return _PullResult(0, 0, {});

    int pulled = 0;
    int conflicts = 0;
    final pulledIds = <String, Set<String>>{};

    for (final (table, idField) in _entities) {
      try {
        // 构建查询：先 filter（gt/eq），后 transform（order）
        var queryBuilder = client
            .from(table)
            .select('*')
            .eq('user_id', userId);

        if (lastSync != null && lastSync.isNotEmpty) {
          queryBuilder = queryBuilder.gt('update_time', lastSync);
        }

        final List<dynamic> remoteRows = await queryBuilder.order('update_time', ascending: true);
        if (remoteRows.isEmpty) continue;

        final ids = <String>{};
        for (final r in remoteRows) {
          final row = r as Map<String, dynamic>;
          row.remove('user_id');

          final localTime = await _getLocalUpdateTime(table, idField, row[idField] as String);
          if (localTime != null) {
            final localDt = DateTime.parse(localTime);
            final remoteDt = DateTime.parse(row['update_time'] as String);
            if (localDt.compareTo(remoteDt) >= 0) {
              conflicts++;
              continue;
            }
          }

          await AppDatabase.upsertRow(table, row);
          pulled++;
          ids.add(row[idField] as String);
        }

        if (ids.isNotEmpty) {
          pulledIds[table] = ids;
        }
      } catch (_) {
        // 单表拉取失败，继续下一张
      }
    }

    return _PullResult(pulled, conflicts, pulledIds);
  }

  Future<String?> _getLocalUpdateTime(String table, String idField, String id) async {
    final db = await AppDatabase.instance;
    final rows = await db.query(table,
        columns: ['update_time'],
        where: '$idField = ?',
        whereArgs: [id]);
    if (rows.isEmpty) return null;
    return rows.first['update_time'] as String?;
  }

  // ═══════════════════════════════════════════════
  // Tombstones
  // ═══════════════════════════════════════════════

  Future<int> _pullTombstones(String userId) async {
    final client = SupabaseService.client;
    if (client == null) return 0;

    final lastPull = await AppDatabase.getSyncMeta('last_tombstone_pull') ?? '1970-01-01T00:00:00';

    try {
      final tombstones = await client
          .from('sync_tombstone')
          .select('*')
          .eq('user_id', userId)
          .gt('deleted_at', lastPull)
          .order('deleted_at', ascending: true);

      if (tombstones.isEmpty) return 0;

      int count = 0;
      for (final t in tombstones) {
        final tableName = t['table_name'] as String;
        final recordId = t['record_id'] as String;
        try {
          if (tableName == 'note_document') {
            await AppDatabase.deleteVersionsByDocument(recordId);
          }
          await AppDatabase.hardDelete(tableName, recordId);
          count++;
        } catch (_) {}
      }

      final latest = tombstones.last['deleted_at'] as String;
      await AppDatabase.setSyncMeta('last_tombstone_pull', latest);

      // 回收 30 天前旧墓碑
      try {
        final cutoff = DateTime.now()
            .subtract(const Duration(days: AppConstants.tombstoneRetentionDays))
            .toUtc()
            .toIso8601String();
        await client
            .from('sync_tombstone')
            .delete()
            .eq('user_id', userId)
            .lt('deleted_at', cutoff);
      } catch (_) {}

      return count;
    } catch (_) {
      return 0;
    }
  }

  // ═══════════════════════════════════════════════
  // Cleanup expired deletes
  // ═══════════════════════════════════════════════

  Future<int> _cleanupExpiredDeletes(String userId) async {
    final client = SupabaseService.client;
    if (client == null) return 0;

    final cutoff = DateTime.now()
        .subtract(const Duration(seconds: AppConstants.cleanupSeconds))
        .toUtc()
        .toIso8601String();

    int cleaned = 0;

    // Cleanup documents
    final expiredDocs = await (await AppDatabase.instance).query('note_document',
        where: 'deleted = 2 AND update_time < ?',
        whereArgs: [cutoff]);

    for (final doc in expiredDocs) {
      try {
        await client.from('note_document_version').delete().eq('document_id', doc['id'] as String).eq('user_id', userId);
        await client.from('note_document').delete().eq('id', doc['id'] as String).eq('user_id', userId);
        await client.from('sync_tombstone').insert({
          'table_name': 'note_document',
          'record_id': doc['id'],
          'user_id': userId,
        });
        await AppDatabase.deleteVersionsByDocument(doc['id'] as String);
        await AppDatabase.hardDelete('note_document', doc['id'] as String);
        cleaned++;
      } catch (_) {}
    }

    // Cleanup folders
    final expiredFolders = await (await AppDatabase.instance).query('note_folder',
        where: 'deleted = 2 AND update_time < ?',
        whereArgs: [cutoff]);

    for (final folder in expiredFolders) {
      try {
        await client.from('note_folder').delete().eq('id', folder['id'] as String).eq('user_id', userId);
        await client.from('sync_tombstone').insert({
          'table_name': 'note_folder',
          'record_id': folder['id'],
          'user_id': userId,
        });
        await AppDatabase.hardDelete('note_folder', folder['id'] as String);
        cleaned++;
      } catch (_) {}
    }

    return cleaned;
  }

  // ═══════════════════════════════════════════════
  // Status
  // ═══════════════════════════════════════════════

  SyncStatus get status => SyncStatus(
        isSyncing: _isSyncing,
        lastSyncTime: _lastSyncTime,
      );

  void _emitStatus() {
    onStatusChange?.call(status);
  }
}

// ═══════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════

class SyncStatus {
  final bool isSyncing;
  final String? lastSyncTime;

  const SyncStatus({required this.isSyncing, this.lastSyncTime});
}

class SyncResult {
  final int pushedCount;
  final int pulledCount;
  final int conflictCount;
  final int cleanupCount;
  final int tombstoneCount;
  final String syncTime;

  const SyncResult({
    this.pushedCount = 0,
    this.pulledCount = 0,
    this.conflictCount = 0,
    this.cleanupCount = 0,
    this.tombstoneCount = 0,
    required this.syncTime,
  });
}

class _PullResult {
  final int count;
  final int conflicts;
  final Map<String, Set<String>> pulledIds;

  const _PullResult(this.count, this.conflicts, this.pulledIds);
}
