import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../database/app_database.dart';
import '../models/models.dart';
import '../services/folder_service.dart';
import '../services/document_service.dart';
import '../services/supabase_service.dart';
import '../services/sync_engine.dart';

// ═══════════════════════════════════════════════
// Local mode — skip auth, use app without login
// ═══════════════════════════════════════════════

final localModeProvider = StateProvider<bool>((ref) => false);

// ═══════════════════════════════════════════════
// Database provider
// ═══════════════════════════════════════════════

final databaseReadyProvider = FutureProvider<void>((ref) async {
  await AppDatabase.instance;
});

// ═══════════════════════════════════════════════
// Auth provider
// ═══════════════════════════════════════════════

final authStateProvider = StreamProvider<bool>((ref) {
  return SupabaseService.authStateChanges.map((event) {
    return SupabaseService.isLoggedIn;
  });
});

// ═══════════════════════════════════════════════
// Folder providers
// ═══════════════════════════════════════════════

final folderTreeProvider = FutureProvider<List<FolderNode>>((ref) async {
  return await FolderService.getFolderTree();
});

final folderListProvider = FutureProvider<List<Folder>>((ref) async {
  return await FolderService.getFolders();
});

/// Trigger folder refresh
final folderRefreshProvider = StateProvider<int>((ref) => 0);

final refreshedFolderTreeProvider = FutureProvider<List<FolderNode>>((ref) {
  ref.watch(folderRefreshProvider);
  return FolderService.getFolderTree();
});

// ═══════════════════════════════════════════════
// Document providers
// ═══════════════════════════════════════════════

final documentsByFolderProvider =
    FutureProvider.family<List<Document>, String>((ref, folderId) async {
  return await DocumentService.listByFolder(folderId);
});

final allDocumentsProvider = FutureProvider<List<Document>>((ref) async {
  return await DocumentService.listAll();
});

final trashDocumentsProvider = FutureProvider<List<Document>>((ref) async {
  return await DocumentService.listTrash();
});

final documentProvider = FutureProvider.family<Document, String>((ref, id) async {
  return await DocumentService.getById(id);
});

final searchResultsProvider =
    FutureProvider.family<List<Document>, String>((ref, keyword) async {
  if (keyword.trim().isEmpty) return [];
  return await DocumentService.search(keyword);
});

final documentRefreshProvider = StateProvider<int>((ref) => 0);

final refreshedAllDocumentsProvider = FutureProvider<List<Document>>((ref) {
  ref.watch(documentRefreshProvider);
  return DocumentService.listAll();
});

final refreshedDocumentsByFolderProvider =
    FutureProvider.family<List<Document>, String>((ref, folderId) {
  ref.watch(documentRefreshProvider);
  return DocumentService.listByFolder(folderId);
});

// ═══════════════════════════════════════════════
// Selected folder/document state
// ═══════════════════════════════════════════════

final selectedFolderIdProvider = StateProvider<String?>((ref) => null);
final selectedDocumentIdProvider = StateProvider<String?>((ref) => null);

/// 当前目录下的文档列表
final currentFolderDocumentsProvider = FutureProvider<List<Document>>((ref) {
  final folderId = ref.watch(selectedFolderIdProvider);
  ref.watch(documentRefreshProvider);
  if (folderId == null) return DocumentService.listAll();
  return DocumentService.listByFolder(folderId);
});

// ═══════════════════════════════════════════════
// Sync provider
// ═══════════════════════════════════════════════

final syncEngineProvider = Provider<SyncEngine>((ref) {
  final engine = SyncEngine();
  ref.onDispose(() => engine.stop());
  return engine;
});

final syncStatusProvider = StateNotifierProvider<SyncStatusNotifier, SyncStatus>((ref) {
  return SyncStatusNotifier();
});

class SyncStatusNotifier extends StateNotifier<SyncStatus> {
  SyncStatusNotifier() : super(const SyncStatus(isSyncing: false));

  void update(SyncStatus newStatus) {
    state = newStatus;
  }
}

/// 初始化 sync engine 并绑定状态
/// 注意：无网络 / Supabase 未配置时静默失败，APP 仍可纯本地使用
final syncInitProvider = FutureProvider<void>((ref) async {
  try {
    await SupabaseService.initialize();
  } catch (_) {
    // Supabase 可能未配置或无网络 — 纯本地模式
    return;
  }
  final engine = ref.read(syncEngineProvider);
  final statusNotifier = ref.read(syncStatusProvider.notifier);

  engine.onStatusChange = (status) {
    statusNotifier.update(status);
  };

  engine.onDataChange = () {
    ref.read(folderRefreshProvider.notifier).state++;
    ref.read(documentRefreshProvider.notifier).state++;
  };

  try {
    await engine.start();
  } catch (_) {
    // Sync engine 启动失败 — 纯本地模式
  }
});
