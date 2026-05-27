import 'package:html/parser.dart' show parse;
import '../database/app_database.dart';
import '../models/models.dart';

/// 文稿业务逻辑 — 镜像桌面端 documentService.cjs
class DocumentService {
  /// 按目录列出
  static Future<List<Document>> listByFolder(String folderId) async {
    return await AppDatabase.getDocumentsByFolder(folderId);
  }

  /// 全部文档
  static Future<List<Document>> listAll() async {
    return await AppDatabase.getAllDocuments();
  }

  /// 回收站
  static Future<List<Document>> listTrash() async {
    return await AppDatabase.getTrashDocuments();
  }

  /// 搜索
  static Future<List<Document>> search(String keyword) async {
    return await AppDatabase.searchDocuments(keyword);
  }

  /// 获取单篇
  static Future<Document> getById(String id) async {
    final doc = await AppDatabase.getDocument(id);
    if (doc == null) throw Exception('文稿不存在');
    return doc;
  }

  /// 创建
  static Future<Document> create(String title, String folderId) async {
    if (title.trim().isEmpty) throw Exception('标题不能为空');
    final doc = Document.newDocument(title: title.trim(), folderId: folderId);
    await AppDatabase.insertDocument(doc);
    return doc;
  }

  /// 重命名
  static Future<void> rename(String id, String title) async {
    final doc = await AppDatabase.getDocument(id);
    if (doc == null) throw Exception('文稿不存在');
    await AppDatabase.updateDocument(doc.copyWith(title: title, updateTime: _now()));
  }

  /// 更新内容（自动提取标题）
  static Future<void> updateContent(String id, String content) async {
    final doc = await AppDatabase.getDocument(id);
    if (doc == null) throw Exception('文稿不存在');
    final title = _extractTitle(content);
    await AppDatabase.updateDocumentContent(id, content, title);
  }

  /// 移入回收站
  static Future<void> remove(String id) async {
    await AppDatabase.softDeleteDocument(id);
  }

  /// 从回收站恢复
  static Future<void> restore(String id) async {
    await AppDatabase.restoreDocument(id);
  }

  /// 标记待清理
  static Future<void> hardDelete(String id) async {
    await AppDatabase.markDocumentCleanup(id);
  }

  /// 清空回收站
  static Future<void> emptyTrash() async {
    final trash = await AppDatabase.getTrashDocuments();
    for (final doc in trash) {
      await AppDatabase.markDocumentCleanup(doc.id);
    }
  }

  /// 从 HTML 内容中提取首行作为标题
  static String _extractTitle(String htmlContent) {
    if (htmlContent.isEmpty) return '新文稿';
    try {
      final document = parse(htmlContent);
      final text = document.body?.text ?? '';
      for (final line in text.split('\n')) {
        final t = line.trim();
        if (t.isNotEmpty) {
          return t.length > 40 ? '${t.substring(0, 40)}…' : t;
        }
      }
    } catch (_) {
      // Fallback: take first non-empty line
    }
    return '新文稿';
  }
}

String _now() => DateTime.now().toUtc().toIso8601String();
