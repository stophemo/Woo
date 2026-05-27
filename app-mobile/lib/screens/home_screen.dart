import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../config/constants.dart';
import '../models/models.dart';
import '../services/document_service.dart';
import '../services/folder_service.dart';
import '../providers/providers.dart';
import '../widgets/folder_tree_tile.dart';
import '../widgets/sync_status_banner.dart';
import 'editor_screen.dart';

/// 主页 — 左侧文件夹抽屉 / 右侧文档列表
class HomeScreen extends ConsumerWidget {
  const HomeScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final selectedFolder = ref.watch(selectedFolderIdProvider);

    return Scaffold(
      appBar: AppBar(
        title: Text(AppConstants.appName),
        leading: Builder(
          builder: (ctx) => IconButton(
            icon: const Icon(Icons.folder_outlined),
            onPressed: () => Scaffold.of(ctx).openDrawer(),
            tooltip: '目录树',
          ),
        ),
        actions: [
          // Search
          IconButton(
            icon: const Icon(Icons.search),
            onPressed: () => _showSearch(context, ref),
            tooltip: '搜索',
          ),
          // New document
          IconButton(
            icon: const Icon(Icons.add),
            onPressed: selectedFolder != null
                ? () => _createDocument(context, ref)
                : null,
            tooltip: '新建笔记',
          ),
        ],
      ),
      drawer: _FolderDrawer(),
      body: Column(
        children: [
          const SyncStatusBanner(),
          Expanded(
            child: selectedFolder == null
                ? _AllDocumentsView()
                : _FolderDocumentsView(folderId: selectedFolder),
          ),
        ],
      ),
    );
  }

  void _showSearch(BuildContext context, WidgetRef ref) {
    showSearch(
      context: context,
      delegate: _DocumentSearchDelegate(ref),
    );
  }

  Future<void> _createDocument(BuildContext context, WidgetRef ref) async {
    final folderId = ref.read(selectedFolderIdProvider);
    if (folderId == null) return;

    final titleCtrl = TextEditingController();
    final title = await showDialog<String>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('新建笔记'),
        content: TextField(
          controller: titleCtrl,
          autofocus: true,
          decoration: const InputDecoration(
            hintText: '输入标题…',
            border: OutlineInputBorder(),
          ),
          textInputAction: TextInputAction.done,
          onSubmitted: (v) => Navigator.of(ctx).pop(v),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('取消'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(titleCtrl.text.trim()),
            child: const Text('创建'),
          ),
        ],
      ),
    );

    if (title == null || title.isEmpty) return;

    try {
      await DocumentService.create(title, folderId);
      ref.read(documentRefreshProvider.notifier).state++;
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('创建失败: $e')),
        );
      }
    }
  }
}

// ═══════════════════════════════════════════════
// Folder Drawer
// ═══════════════════════════════════════════════

class _FolderDrawer extends ConsumerWidget {
  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final treeAsync = ref.watch(refreshedFolderTreeProvider);
    final theme = Theme.of(context);

    return Drawer(
      child: SafeArea(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
              child: Row(
                children: [
                  Text('目录', style: theme.textTheme.titleMedium),
                  const Spacer(),
                  IconButton(
                    icon: const Icon(Icons.create_new_folder_outlined),
                    tooltip: '新建目录',
                    onPressed: () => _createFolder(context, ref),
                  ),
                ],
              ),
            ),
            const Divider(height: 1),
            // All notes option
            ListTile(
              leading: const Icon(Icons.notes),
              title: const Text('全部笔记'),
              selected: ref.watch(selectedFolderIdProvider) == null,
              onTap: () {
                ref.read(selectedFolderIdProvider.notifier).state = null;
                Navigator.of(context).pop();
              },
            ),
            const Divider(),
            Expanded(
              child: treeAsync.when(
                data: (nodes) {
                  if (nodes.isEmpty) {
                    return const Center(child: Text('暂无目录'));
                  }
                  return ListView(
                    children: nodes.map((n) => FolderTreeTile(node: n, depth: 0)).toList(),
                  );
                },
                loading: () => const Center(child: CircularProgressIndicator()),
                error: (e, _) => Center(child: Text('加载失败: $e')),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _createFolder(BuildContext context, WidgetRef ref) async {
    final nameCtrl = TextEditingController();
    final name = await showDialog<String>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('新建目录'),
        content: TextField(
          controller: nameCtrl,
          autofocus: true,
          decoration: const InputDecoration(
            hintText: '目录名称…',
            border: OutlineInputBorder(),
          ),
          textInputAction: TextInputAction.done,
          onSubmitted: (v) => Navigator.of(ctx).pop(v),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('取消'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(nameCtrl.text.trim()),
            child: const Text('创建'),
          ),
        ],
      ),
    );

    if (name == null || name.isEmpty) return;

    try {
      await FolderService.createFolder(name);
      ref.read(folderRefreshProvider.notifier).state++;
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('已创建目录「$name」')),
        );
      }
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('创建失败: $e')),
        );
      }
    }
  }
}

// ═══════════════════════════════════════════════
// Document Views
// ═══════════════════════════════════════════════

class _AllDocumentsView extends ConsumerWidget {
  const _AllDocumentsView();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final docsAsync = ref.watch(refreshedAllDocumentsProvider);

    return docsAsync.when(
      data: (docs) => _DocumentList(documents: docs),
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (e, _) => Center(child: Text('加载失败: $e')),
    );
  }
}

class _FolderDocumentsView extends ConsumerWidget {
  final String folderId;
  const _FolderDocumentsView({required this.folderId});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final docsAsync = ref.watch(refreshedDocumentsByFolderProvider(folderId));

    return docsAsync.when(
      data: (docs) => _DocumentList(documents: docs),
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (e, _) => Center(child: Text('加载失败: $e')),
    );
  }
}

// ═══════════════════════════════════════════════
// Document List
// ═══════════════════════════════════════════════

class _DocumentList extends ConsumerWidget {
  final List<Document> documents;

  const _DocumentList({required this.documents});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (documents.isEmpty) {
      return Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.notes, size: 64, color: Theme.of(context).colorScheme.onSurfaceVariant.withOpacity(0.4)),
            const SizedBox(height: 16),
            Text(
              '暂无笔记',
              style: Theme.of(context).textTheme.titleMedium?.copyWith(
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
                  ),
            ),
            const SizedBox(height: 8),
            Text(
              '选择目录后点击 + 创建',
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                    color: Theme.of(context).colorScheme.onSurfaceVariant.withOpacity(0.7),
                  ),
            ),
          ],
        ),
      );
    }

    return RefreshIndicator(
      onRefresh: () async {
        ref.read(documentRefreshProvider.notifier).state++;
      },
      child: ListView.separated(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        itemCount: documents.length,
        separatorBuilder: (_, __) => const SizedBox(height: 4),
        itemBuilder: (context, index) {
          final doc = documents[index];
          return _DocumentCard(doc: doc);
        },
      ),
    );
  }
}

class _DocumentCard extends ConsumerWidget {
  final Document doc;
  const _DocumentCard({required this.doc});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final theme = Theme.of(context);
    final updated = _formatDate(doc.updateTime);

    return Card(
      child: InkWell(
        borderRadius: BorderRadius.circular(12),
        onTap: () {
          Navigator.of(context).push(
            MaterialPageRoute(
              builder: (_) => EditorScreen(documentId: doc.id),
            ),
          );
        },
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      doc.title,
                      style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.w600),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    if (doc.content.isNotEmpty) ...[
                      const SizedBox(height: 4),
                      Text(
                        _stripHtml(doc.content),
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: theme.colorScheme.onSurfaceVariant,
                        ),
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                    const SizedBox(height: 4),
                    Text(
                      updated,
                      style: theme.textTheme.labelSmall?.copyWith(
                        color: theme.colorScheme.onSurfaceVariant.withOpacity(0.6),
                      ),
                    ),
                  ],
                ),
              ),
              PopupMenuButton<String>(
                icon: Icon(Icons.more_vert,
                    color: theme.colorScheme.onSurfaceVariant.withOpacity(0.6)),
                onSelected: (action) async {
                  if (action == 'rename') {
                    await _rename(context, ref);
                  } else if (action == 'delete') {
                    await _delete(context, ref);
                  }
                },
                itemBuilder: (_) => [
                  const PopupMenuItem(value: 'rename', child: Text('重命名')),
                  const PopupMenuItem(value: 'delete', child: Text('删除')),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _rename(BuildContext context, WidgetRef ref) async {
    final ctrl = TextEditingController(text: doc.title);
    final newTitle = await showDialog<String>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('重命名'),
        content: TextField(
          controller: ctrl,
          autofocus: true,
          decoration: const InputDecoration(border: OutlineInputBorder()),
          textInputAction: TextInputAction.done,
          onSubmitted: (v) => Navigator.of(ctx).pop(v),
        ),
        actions: [
          TextButton(onPressed: () => Navigator.of(ctx).pop(), child: const Text('取消')),
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(ctrl.text.trim()),
            child: const Text('确定'),
          ),
        ],
      ),
    );
    if (newTitle == null || newTitle.isEmpty) return;
    try {
      await DocumentService.rename(doc.id, newTitle);
      ref.read(documentRefreshProvider.notifier).state++;
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  Future<void> _delete(BuildContext context, WidgetRef ref) async {
    final confirm = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('删除笔记'),
        content: Text('将「${doc.title}」移入回收站？'),
        actions: [
          TextButton(onPressed: () => Navigator.of(ctx).pop(false), child: const Text('取消')),
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            style: FilledButton.styleFrom(
              backgroundColor: Theme.of(context).colorScheme.error,
            ),
            child: const Text('删除'),
          ),
        ],
      ),
    );
    if (confirm != true) return;
    try {
      await DocumentService.remove(doc.id);
      ref.read(documentRefreshProvider.notifier).state++;
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }
}

// ═══════════════════════════════════════════════
// Search Delegate
// ═══════════════════════════════════════════════

class _DocumentSearchDelegate extends SearchDelegate<String?> {
  final WidgetRef ref;

  _DocumentSearchDelegate(this.ref);

  @override
  List<Widget>? buildActions(BuildContext context) {
    return [
      if (query.isNotEmpty)
        IconButton(icon: const Icon(Icons.clear), onPressed: () => query = ''),
    ];
  }

  @override
  Widget? buildLeading(BuildContext context) {
    return IconButton(
      icon: const Icon(Icons.arrow_back),
      onPressed: () => close(context, null),
    );
  }

  @override
  Widget buildResults(BuildContext context) => _buildList(context);

  @override
  Widget buildSuggestions(BuildContext context) => _buildList(context);

  Widget _buildList(BuildContext context) {
    if (query.isEmpty) {
      return Center(
        child: Text(
          '搜索笔记标题或内容…',
          style: Theme.of(context).textTheme.bodyLarge?.copyWith(
                color: Theme.of(context).colorScheme.onSurfaceVariant,
              ),
        ),
      );
    }

    return FutureBuilder<List<Document>>(
      future: DocumentService.search(query),
      builder: (context, snapshot) {
        if (snapshot.connectionState == ConnectionState.waiting) {
          return const Center(child: CircularProgressIndicator());
        }
        if (snapshot.hasError) {
          return Center(child: Text('搜索失败: ${snapshot.error}'));
        }
        final docs = snapshot.data ?? [];
        if (docs.isEmpty) {
          return const Center(child: Text('未找到匹配的笔记'));
        }
        return ListView.separated(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          itemCount: docs.length,
          separatorBuilder: (_, __) => const SizedBox(height: 4),
          itemBuilder: (_, i) => ListTile(
            title: Text(docs[i].title, maxLines: 1, overflow: TextOverflow.ellipsis),
            subtitle: Text(_stripHtml(docs[i].content),
                maxLines: 2, overflow: TextOverflow.ellipsis),
            onTap: () {
              close(context, null);
              Navigator.of(context).push(
                MaterialPageRoute(
                  builder: (_) => EditorScreen(documentId: docs[i].id),
                ),
              );
            },
          ),
        );
      },
    );
  }
}

// ═══════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════

String _formatDate(String iso) {
  try {
    final dt = DateTime.parse(iso).toLocal();
    final now = DateTime.now();
    final diff = now.difference(dt);
    if (diff.inMinutes < 1) return '刚刚';
    if (diff.inHours < 1) return '${diff.inMinutes} 分钟前';
    if (diff.inDays < 1) return '${diff.inHours} 小时前';
    if (diff.inDays < 7) return '${diff.inDays} 天前';
    return '${dt.month}/${dt.day}';
  } catch (_) {
    return '';
  }
}

String _stripHtml(String html) {
  if (html.isEmpty) return '';
  try {
    final reg = RegExp(r'<[^>]*>', multiLine: true);
    return html.replaceAll(reg, ' ').replaceAll(RegExp(r'\s+'), ' ').trim();
  } catch (_) {
    return html;
  }
}
