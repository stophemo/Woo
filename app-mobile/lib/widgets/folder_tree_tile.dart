import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../models/models.dart';
import '../providers/providers.dart';

/// 可展开的目录树节点
class FolderTreeTile extends ConsumerWidget {
  final FolderNode node;
  final int depth;

  const FolderTreeTile({
    super.key,
    required this.node,
    this.depth = 0,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final selectedFolderId = ref.watch(selectedFolderIdProvider);
    final isSelected = selectedFolderId == node.folder.id;
    final hasChildren = node.children.isNotEmpty;
    final theme = Theme.of(context);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        // Folder item
        InkWell(
          onTap: () {
            ref.read(selectedFolderIdProvider.notifier).state = node.folder.id;
            Navigator.of(context).pop(); // close drawer
          },
          child: Padding(
            padding: EdgeInsets.only(left: 16.0 + depth * 20, right: 8),
            child: SizedBox(
              height: 44,
              child: Row(
                children: [
                  Icon(
                    hasChildren ? Icons.folder : Icons.folder_outlined,
                    size: 20,
                    color: isSelected
                        ? theme.colorScheme.primary
                        : theme.colorScheme.onSurfaceVariant,
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Text(
                      node.folder.name,
                      style: TextStyle(
                        fontSize: 14,
                        fontWeight: isSelected ? FontWeight.w600 : FontWeight.normal,
                        color: isSelected
                            ? theme.colorScheme.primary
                            : theme.colorScheme.onSurface,
                      ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  if (isSelected)
                    Container(
                      width: 4,
                      height: 24,
                      decoration: BoxDecoration(
                        color: theme.colorScheme.primary,
                        borderRadius: BorderRadius.circular(2),
                      ),
                    ),
                ],
              ),
            ),
          ),
        ),
        // Children
        if (hasChildren)
          ...node.children.map(
            (child) => FolderTreeTile(node: child, depth: depth + 1),
          ),
      ],
    );
  }
}
