import 'package:sqflite/sqflite.dart';
import 'package:path/path.dart' as p;
import '../config/constants.dart';
import '../models/models.dart';

/// SQLite 数据库封装 — 与桌面端完全兼容的表结构
class AppDatabase {
  static Database? _db;

  static Future<Database> get instance async {
    if (_db != null) return _db!;
    _db = await _init();
    return _db!;
  }

  static Future<Database> _init() async {
    final dir = await getDatabasesPath();
    final dbPath = p.join(dir, AppConstants.dbName);
    final db = await openDatabase(
      dbPath,
      version: AppConstants.dbVersion,
      onCreate: _onCreate,
      onUpgrade: _onUpgrade,
      // WAL mode for better concurrent performance
      // NOTE: sqflite doesn't directly expose WAL on all platforms
    );
    // Enable foreign keys
    await db.execute('PRAGMA foreign_keys = ON');
    return db;
  }

  static Future<void> _onCreate(Database db, int version) async {
    final batch = db.batch();
    batch.execute('''
      CREATE TABLE IF NOT EXISTS note_folder (
        id TEXT PRIMARY KEY,
        parent_id TEXT,
        name TEXT NOT NULL,
        sort_order INTEGER NOT NULL DEFAULT 0,
        create_time TEXT NOT NULL,
        update_time TEXT NOT NULL,
        deleted INTEGER NOT NULL DEFAULT 0,
        is_locked INTEGER NOT NULL DEFAULT 0
      )
    ''');
    batch.execute('CREATE INDEX IF NOT EXISTS idx_folder_parent ON note_folder(parent_id)');

    batch.execute('''
      CREATE TABLE IF NOT EXISTS note_document (
        id TEXT PRIMARY KEY,
        folder_id TEXT NOT NULL,
        title TEXT NOT NULL,
        content TEXT,
        branch_name TEXT,
        sort_order INTEGER NOT NULL DEFAULT 0,
        create_time TEXT NOT NULL,
        update_time TEXT NOT NULL,
        deleted INTEGER NOT NULL DEFAULT 0,
        is_locked INTEGER NOT NULL DEFAULT 0
      )
    ''');
    batch.execute('CREATE INDEX IF NOT EXISTS idx_doc_folder ON note_document(folder_id)');

    batch.execute('''
      CREATE TABLE IF NOT EXISTS note_document_version (
        id TEXT PRIMARY KEY,
        document_id TEXT NOT NULL,
        version_no INTEGER NOT NULL,
        title TEXT,
        content TEXT,
        content_hash TEXT,
        change_type TEXT NOT NULL DEFAULT 'auto',
        operator_id TEXT,
        create_time TEXT NOT NULL,
        update_time TEXT NOT NULL,
        deleted INTEGER NOT NULL DEFAULT 0,
        UNIQUE(document_id, version_no)
      )
    ''');
    batch.execute('CREATE INDEX IF NOT EXISTS idx_ver_doc ON note_document_version(document_id)');

    batch.execute('''
      CREATE TABLE IF NOT EXISTS sync_meta (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
      )
    ''');

    await batch.commit(noResult: true);
  }

  static Future<void> _onUpgrade(Database db, int oldV, int newV) async {
    // Future migrations go here
  }

  // ═══════════════════════════════════════════════
  // Folder DAO
  // ═══════════════════════════════════════════════

  static Future<List<Folder>> getFolders() async {
    final db = await instance;
    final rows = await db.query('note_folder', where: 'deleted = 0', orderBy: 'sort_order ASC, create_time ASC');
    return rows.map((m) => Folder.fromMap(m)).toList();
  }

  static Future<Folder?> getFolder(String id) async {
    final db = await instance;
    final rows = await db.query('note_folder', where: 'id = ?', whereArgs: [id]);
    return rows.isEmpty ? null : Folder.fromMap(rows.first);
  }

  static Future<void> insertFolder(Folder folder) async {
    final db = await instance;
    await db.insert('note_folder', folder.toMap(), conflictAlgorithm: ConflictAlgorithm.replace);
  }

  static Future<void> updateFolder(Folder folder) async {
    final db = await instance;
    await db.update('note_folder', folder.toMap(), where: 'id = ?', whereArgs: [folder.id]);
  }

  static Future<void> updateFolderName(String id, String name) async {
    final db = await instance;
    await db.update('note_folder', {'name': name, 'update_time': _now()}, where: 'id = ?', whereArgs: [id]);
  }

  static Future<void> softDeleteFolder(String id) async {
    final db = await instance;
    await db.update('note_folder', {'deleted': 1, 'update_time': _now()}, where: 'id = ?', whereArgs: [id]);
  }

  static Future<void> hardDeleteFolder(String id) async {
    final db = await instance;
    await db.delete('note_folder', where: 'id = ?', whereArgs: [id]);
  }

  // ═══════════════════════════════════════════════
  // Document DAO
  // ═══════════════════════════════════════════════

  static Future<List<Document>> getDocumentsByFolder(String folderId) async {
    final db = await instance;
    final rows = await db.query('note_document',
        where: 'folder_id = ? AND deleted = 0',
        whereArgs: [folderId],
        orderBy: 'update_time DESC');
    return rows.map((m) => Document.fromMap(m)).toList();
  }

  static Future<List<Document>> getAllDocuments() async {
    final db = await instance;
    final rows = await db.query('note_document',
        where: 'deleted = 0', orderBy: 'update_time DESC');
    return rows.map((m) => Document.fromMap(m)).toList();
  }

  static Future<List<Document>> getTrashDocuments() async {
    final db = await instance;
    final rows = await db.query('note_document',
        where: 'deleted = 1', orderBy: 'update_time DESC');
    return rows.map((m) => Document.fromMap(m)).toList();
  }

  static Future<Document?> getDocument(String id) async {
    final db = await instance;
    final rows = await db.query('note_document', where: 'id = ?', whereArgs: [id]);
    return rows.isEmpty ? null : Document.fromMap(rows.first);
  }

  static Future<List<Document>> searchDocuments(String keyword) async {
    final db = await instance;
    final like = '%$keyword%';
    final rows = await db.query('note_document',
        where: 'deleted = 0 AND (title LIKE ? OR content LIKE ?)',
        whereArgs: [like, like],
        orderBy: 'update_time DESC');
    return rows.map((m) => Document.fromMap(m)).toList();
  }

  static Future<void> insertDocument(Document doc) async {
    final db = await instance;
    await db.insert('note_document', doc.toMap(), conflictAlgorithm: ConflictAlgorithm.replace);
  }

  static Future<void> updateDocument(Document doc) async {
    final db = await instance;
    await db.update('note_document', doc.toMap(), where: 'id = ?', whereArgs: [doc.id]);
  }

  static Future<void> updateDocumentContent(String id, String content, String title) async {
    final db = await instance;
    await db.update('note_document',
        {'content': content, 'title': title, 'update_time': _now()},
        where: 'id = ?',
        whereArgs: [id]);
  }

  static Future<void> softDeleteDocument(String id) async {
    final db = await instance;
    await db.update('note_document', {'deleted': 1, 'update_time': _now()}, where: 'id = ?', whereArgs: [id]);
  }

  static Future<void> restoreDocument(String id) async {
    final db = await instance;
    await db.update('note_document', {'deleted': 0, 'update_time': _now()}, where: 'id = ?', whereArgs: [id]);
  }

  static Future<void> markDocumentCleanup(String id) async {
    final db = await instance;
    await db.update('note_document', {'deleted': 2, 'update_time': _now()}, where: 'id = ?', whereArgs: [id]);
  }

  // ═══════════════════════════════════════════════
  // Sync Meta DAO
  // ═══════════════════════════════════════════════

  static Future<String?> getSyncMeta(String key) async {
    final db = await instance;
    final rows = await db.query('sync_meta', where: 'key = ?', whereArgs: [key]);
    return rows.isEmpty ? null : rows.first['value'] as String;
  }

  static Future<void> setSyncMeta(String key, String value) async {
    final db = await instance;
    await db.insert('sync_meta', {'key': key, 'value': value},
        conflictAlgorithm: ConflictAlgorithm.replace);
  }

  // ═══════════════════════════════════════════════
  // Raw query for sync (matching desktop's dynamic upsert)
  // ═══════════════════════════════════════════════

  /// 直接更新/插入一行（同步引擎用）
  static Future<void> upsertRow(String table, Map<String, dynamic> row) async {
    final db = await instance;
    await db.insert(table, row, conflictAlgorithm: ConflictAlgorithm.replace);
  }

  /// 查询表 update_time > since 的记录（同步引擎用）
  static Future<List<Map<String, dynamic>>> queryChangesSince(String table, String since) async {
    final db = await instance;
    return await db.query(table,
        where: 'update_time > ?',
        whereArgs: [since],
        orderBy: 'update_time ASC');
  }

  /// 硬删除一行（墓碑同步用）
  static Future<void> hardDelete(String table, String id) async {
    final db = await instance;
    await db.delete(table, where: 'id = ?', whereArgs: [id]);
  }

  /// 删除关联的版本（级联删除）
  static Future<void> deleteVersionsByDocument(String documentId) async {
    final db = await instance;
    await db.delete('note_document_version',
        where: 'document_id = ?', whereArgs: [documentId]);
  }

  /// 关闭并清理
  static Future<void> close() async {
    final db = _db;
    if (db != null) {
      await db.close();
      _db = null;
    }
  }
}

String _now() => DateTime.now().toUtc().toIso8601String();
