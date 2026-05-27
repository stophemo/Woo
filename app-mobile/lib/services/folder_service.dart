import '../database/app_database.dart';
import '../models/models.dart';

/// 目录业务逻辑 — 镜像桌面端 folderService.cjs
class FolderService {
  /// 获取活跃目录的树形列表
  static Future<List<FolderNode>> getFolderTree() async {
    final folders = await AppDatabase.getFolders();
    return buildFolderTree(folders);
  }

  /// 获取活跃目录的平铺列表
  static Future<List<Folder>> getFolders() async {
    return await AppDatabase.getFolders();
  }

  /// 创建目录（同级同名检查 + 软删除恢复）
  static Future<String> createFolder(String name, {String? parentId}) async {
    if (name.trim().isEmpty) throw Exception('目录名不能为空');

    // 检查同级同名（含软删除）
    // Note: full scan approach — in production, push this to SQL
    final all = await AppDatabase.getFolders();
    final siblings = all.where((f) => f.parentId == parentId).toList();
    final active = siblings.where((f) => f.name == name && f.deleted == 0).toList();
    if (active.isNotEmpty) throw Exception('同级目录名不能重复');

    final softDeleted = siblings.where((f) => f.name == name && f.deleted == 1).toList();
    if (softDeleted.isNotEmpty) {
      // 恢复软删除的目录
      final restored = softDeleted.first.copyWith(deleted: 0, updateTime: DateTime.now().toUtc().toIso8601String());
      await AppDatabase.updateFolder(restored);
      return restored.id;
    }

    final folder = Folder.newFolder(name: name.trim(), parentId: parentId);
    await AppDatabase.insertFolder(folder);
    return folder.id;
  }

  /// 重命名
  static Future<void> renameFolder(String id, String newName) async {
    if (newName.trim().isEmpty) throw Exception('目录名不能为空');
    await AppDatabase.updateFolderName(id, newName.trim());
  }

  /// 软删除目录（级联处理文档和子目录）
  static Future<void> deleteFolder(String folderId) async {
    final folder = await AppDatabase.getFolder(folderId);
    if (folder == null || folder.deleted != 0) throw Exception('目录不存在');

    // 收集所有后代 ID
    final all = await AppDatabase.getFolders();
    final allDocs = await AppDatabase.getAllDocuments();

    final ids = <String>[];
    void walk(String fid) {
      ids.add(fid);
      for (final f in all) {
        if (f.parentId == fid) walk(f.id);
      }
    }
    walk(folderId);

    final now = DateTime.now().toUtc().toIso8601String();

    for (final id in ids) {
      // 子树下文档入废纸篓
      final docs = allDocs.where((d) => d.folderId == id && d.deleted == 0).toList();
      for (final doc in docs) {
        await AppDatabase.updateDocument(doc.copyWith(deleted: 1, updateTime: now));
      }

      // 子树下是否有废纸篓文档
      final trashDocs = allDocs.where((d) => d.folderId == id && d.deleted == 1).toList();
      final targetDeleted = trashDocs.isNotEmpty ? 1 : 2;

      await AppDatabase.updateFolder(
        Folder(
          id: id,
          parentId: null,
          name: '',
          createTime: '',
          updateTime: now,
          deleted: targetDeleted,
        ),
      );
    }
  }
}
