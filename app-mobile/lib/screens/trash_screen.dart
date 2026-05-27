import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../models/models.dart';
import '../services/document_service.dart';
import '../providers/providers.dart';

/// 回收站页面
class TrashScreen extends ConsumerWidget {
  const TrashScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final trashAsync = ref.watch(trashDocumentsProvider);
    final theme = Theme.of(context);

    return Scaffold(
      appBar: AppBar(
        title: const Text('回收站'),
        actions: [
          IconButton(
            icon: const Icon(Icons.delete_sweep),
            tooltip: '清空回收站',
            onPressed: () => _emptyTrash(context, ref),
          ),
        ],
      ),
      body: trashAsync.when(
        data: (docs) {
          if (docs.isEmpty) {
            return Center(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Icon(Icons.delete_outline,
                      size: 64,
                      color: theme.colorScheme.onSurfaceVariant.withOpacity(0.4)),
                  const SizedBox(height: 16),
                  Text(
                    '回收站为空',
                    style: theme.textTheme.titleMedium?.copyWith(
                      color: theme.colorScheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ),
            );
          }

          return RefreshIndicator(
            onRefresh: () async => ref.read(documentRefreshProvider.notifier).state++,
            child: ListView.separated(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
              itemCount: docs.length,
              separatorBuilder: (_, __) => const SizedBox(height: 4),
              itemBuilder: (_, i) {
                final doc = docs[i];
                return Card(
                  child: ListTile(
                    leading: const Icon(Icons.description_outlined),
                    title: Text(doc.title, maxLines: 1, overflow: TextOverflow.ellipsis),
                    subtitle: Text(
                      _formatDate(doc.updateTime),
                      style: theme.textTheme.labelSmall,
                    ),
                    trailing: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        IconButton(
                          icon: const Icon(Icons.restore),
                          tooltip: '恢复',
                          onPressed: () => _restore(context, ref, doc),
                        ),
                        IconButton(
                          icon: Icon(Icons.delete_forever,
                              color: theme.colorScheme.error),
                          tooltip: '永久删除',
                          onPressed: () => _hardDelete(context, ref, doc),
                        ),
                      ],
                    ),
                  ),
                );
              },
            ),
          );
        },
        loading: () => const Center(child: CircularProgressIndicator()),
        error: (e, _) => Center(child: Text('加载失败: $e')),
      ),
    );
  }

  Future<void> _restore(BuildContext context, WidgetRef ref, Document doc) async {
    try {
      await DocumentService.restore(doc.id);
      ref.read(documentRefreshProvider.notifier).state++;
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('已恢复「${doc.title}」')),
        );
      }
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('恢复失败: $e')),
        );
      }
    }
  }

  Future<void> _hardDelete(BuildContext context, WidgetRef ref, Document doc) async {
    final confirm = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('永久删除'),
        content: Text('确定永久删除「${doc.title}」？此操作不可撤销。'),
        actions: [
          TextButton(onPressed: () => Navigator.of(ctx).pop(false), child: const Text('取消')),
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            style: FilledButton.styleFrom(
              backgroundColor: Theme.of(context).colorScheme.error,
            ),
            child: const Text('永久删除'),
          ),
        ],
      ),
    );
    if (confirm != true) return;
    try {
      await DocumentService.hardDelete(doc.id);
      ref.read(documentRefreshProvider.notifier).state++;
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('操作失败: $e')),
        );
      }
    }
  }

  Future<void> _emptyTrash(BuildContext context, WidgetRef ref) async {
    final confirm = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('清空回收站'),
        content: const Text('确定清空回收站？所有笔记将被永久删除。'),
        actions: [
          TextButton(onPressed: () => Navigator.of(ctx).pop(false), child: const Text('取消')),
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            style: FilledButton.styleFrom(
              backgroundColor: Theme.of(context).colorScheme.error,
            ),
            child: const Text('清空'),
          ),
        ],
      ),
    );
    if (confirm != true) return;
    try {
      await DocumentService.emptyTrash();
      ref.read(documentRefreshProvider.notifier).state++;
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('回收站已清空')),
        );
      }
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('操作失败: $e')),
        );
      }
    }
  }
}

String _formatDate(String iso) {
  try {
    final dt = DateTime.parse(iso).toLocal();
    return '${dt.year}/${dt.month}/${dt.day} ${dt.hour.toString().padLeft(2, '0')}:${dt.minute.toString().padLeft(2, '0')}';
  } catch (_) {
    return '';
  }
}
