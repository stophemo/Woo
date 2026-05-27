import 'package:uuid/uuid.dart';

/// ISO 8601 timestamp helper
String _now() => DateTime.now().toUtc().toIso8601String();
String _newUuid() => const Uuid().v4();

// ═══════════════════════════════════════════════
// Folder
// ═══════════════════════════════════════════════

class Folder {
  final String id;
  final String? parentId;
  final String name;
  final int sortOrder;
  final String createTime;
  final String updateTime;
  final int deleted; // 0=normal, 1=trash, 2=pending-cleanup
  final int isLocked;

  const Folder({
    required this.id,
    this.parentId,
    required this.name,
    this.sortOrder = 0,
    required this.createTime,
    required this.updateTime,
    this.deleted = 0,
    this.isLocked = 0,
  });

  Map<String, dynamic> toMap() => {
        'id': id,
        'parent_id': parentId,
        'name': name,
        'sort_order': sortOrder,
        'create_time': createTime,
        'update_time': updateTime,
        'deleted': deleted,
        'is_locked': isLocked,
      };

  factory Folder.fromMap(Map<String, dynamic> m) => Folder(
        id: m['id'] as String,
        parentId: m['parent_id'] as String?,
        name: m['name'] as String,
        sortOrder: (m['sort_order'] as num?)?.toInt() ?? 0,
        createTime: m['create_time'] as String? ?? _now(),
        updateTime: m['update_time'] as String? ?? _now(),
        deleted: (m['deleted'] as num?)?.toInt() ?? 0,
        isLocked: (m['is_locked'] as num?)?.toInt() ?? 0,
      );

  Folder copyWith({
    String? id,
    String? parentId,
    String? name,
    int? sortOrder,
    String? createTime,
    String? updateTime,
    int? deleted,
    int? isLocked,
  }) =>
      Folder(
        id: id ?? this.id,
        parentId: parentId ?? this.parentId,
        name: name ?? this.name,
        sortOrder: sortOrder ?? this.sortOrder,
        createTime: createTime ?? this.createTime,
        updateTime: updateTime ?? this.updateTime,
        deleted: deleted ?? this.deleted,
        isLocked: isLocked ?? this.isLocked,
      );

  Folder copyWithUpdated({String? name, int? deleted}) => Folder(
        id: id,
        parentId: parentId,
        name: name ?? this.name,
        sortOrder: sortOrder,
        createTime: createTime,
        updateTime: _now(),
        deleted: deleted ?? this.deleted,
        isLocked: isLocked,
      );

  /// 创建新 Folder 用
  factory Folder.newFolder({
    required String name,
    String? parentId,
  }) =>
      Folder(
        id: _newUuid(),
        parentId: parentId,
        name: name,
        createTime: _now(),
        updateTime: _now(),
      );
}

/// 带 children 的树形 Folder（用于 UI 展示）
class FolderNode {
  final Folder folder;
  final List<FolderNode> children;

  const FolderNode({required this.folder, this.children = const []});

  String get id => folder.id;
  String get name => folder.name;
}

/// 从平铺列表构建树
List<FolderNode> buildFolderTree(List<Folder> flat) {
  final byParent = <String?, List<Folder>>{};
  for (final f in flat) {
    byParent.putIfAbsent(f.parentId, () => []).add(f);
  }

  // 递归构建子树
  FolderNode _buildSub(Folder f) {
    return FolderNode(
      folder: f,
      children: (byParent[f.id] ?? []).map(_buildSub).toList(),
    );
  }

  final rootKids = byParent[null] ?? [];
  return rootKids.map((f) => FolderNode(
        folder: f,
        children: (byParent[f.id] ?? []).map(_buildSub).toList(),
      )).toList();
}

// ═══════════════════════════════════════════════
// Document
// ═══════════════════════════════════════════════

class Document {
  final String id;
  final String folderId;
  final String title;
  final String content;
  final String? branchName;
  final int sortOrder;
  final String createTime;
  final String updateTime;
  final int deleted;
  final int isLocked;

  const Document({
    required this.id,
    required this.folderId,
    required this.title,
    this.content = '',
    this.branchName,
    this.sortOrder = 0,
    required this.createTime,
    required this.updateTime,
    this.deleted = 0,
    this.isLocked = 0,
  });

  Map<String, dynamic> toMap() => {
        'id': id,
        'folder_id': folderId,
        'title': title,
        'content': content,
        'branch_name': branchName,
        'sort_order': sortOrder,
        'create_time': createTime,
        'update_time': updateTime,
        'deleted': deleted,
        'is_locked': isLocked,
      };

  factory Document.fromMap(Map<String, dynamic> m) => Document(
        id: m['id'] as String,
        folderId: m['folder_id'] as String,
        title: m['title'] as String,
        content: m['content'] as String? ?? '',
        branchName: m['branch_name'] as String?,
        sortOrder: (m['sort_order'] as num?)?.toInt() ?? 0,
        createTime: m['create_time'] as String? ?? _now(),
        updateTime: m['update_time'] as String? ?? _now(),
        deleted: (m['deleted'] as num?)?.toInt() ?? 0,
        isLocked: (m['is_locked'] as num?)?.toInt() ?? 0,
      );

  Document copyWith({
    String? id,
    String? folderId,
    String? title,
    String? content,
    String? branchName,
    int? sortOrder,
    String? createTime,
    String? updateTime,
    int? deleted,
    int? isLocked,
  }) =>
      Document(
        id: id ?? this.id,
        folderId: folderId ?? this.folderId,
        title: title ?? this.title,
        content: content ?? this.content,
        branchName: branchName ?? this.branchName,
        sortOrder: sortOrder ?? this.sortOrder,
        createTime: createTime ?? this.createTime,
        updateTime: updateTime ?? this.updateTime,
        deleted: deleted ?? this.deleted,
        isLocked: isLocked ?? this.isLocked,
      );

  factory Document.newDocument({
    required String title,
    required String folderId,
  }) =>
      Document(
        id: _newUuid(),
        folderId: folderId,
        title: title,
        createTime: _now(),
        updateTime: _now(),
      );
}

// ═══════════════════════════════════════════════
// DocumentVersion
// ═══════════════════════════════════════════════

class DocumentVersion {
  final String id;
  final String documentId;
  final int versionNo;
  final String? title;
  final String? content;
  final String? contentHash;
  final String changeType; // 'auto', 'manual', 'restore'
  final String? operatorId;
  final String createTime;
  final String updateTime;
  final int deleted;

  const DocumentVersion({
    required this.id,
    required this.documentId,
    required this.versionNo,
    this.title,
    this.content,
    this.contentHash,
    this.changeType = 'auto',
    this.operatorId,
    required this.createTime,
    required this.updateTime,
    this.deleted = 0,
  });

  Map<String, dynamic> toMap() => {
        'id': id,
        'document_id': documentId,
        'version_no': versionNo,
        'title': title,
        'content': content,
        'content_hash': contentHash,
        'change_type': changeType,
        'operator_id': operatorId,
        'create_time': createTime,
        'update_time': updateTime,
        'deleted': deleted,
      };

  factory DocumentVersion.fromMap(Map<String, dynamic> m) => DocumentVersion(
        id: m['id'] as String,
        documentId: m['document_id'] as String,
        versionNo: (m['version_no'] as num).toInt(),
        title: m['title'] as String?,
        content: m['content'] as String?,
        contentHash: m['content_hash'] as String?,
        changeType: m['change_type'] as String? ?? 'auto',
        operatorId: m['operator_id'] as String?,
        createTime: m['create_time'] as String? ?? _now(),
        updateTime: m['update_time'] as String? ?? _now(),
        deleted: (m['deleted'] as num?)?.toInt() ?? 0,
      );
}
