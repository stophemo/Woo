import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_markdown/flutter_markdown.dart';
import '../models/models.dart';
import '../services/document_service.dart';
import '../providers/providers.dart';

/// 编辑器页面
///
/// 内容存储为 HTML（与桌面端兼容）。
/// 移动端使用 Markdown 编辑，存储时转为 HTML。
class EditorScreen extends ConsumerStatefulWidget {
  final String documentId;

  const EditorScreen({super.key, required this.documentId});

  @override
  ConsumerState<EditorScreen> createState() => _EditorScreenState();
}

class _EditorScreenState extends ConsumerState<EditorScreen> {
  late TextEditingController _controller;
  bool _loading = true;
  Document? _doc;
  String? _error;
  bool _modified = false;
  Timer? _debounceTimer;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController();
    _controller.addListener(_onTextChanged);
    _load();
  }

  @override
  void dispose() {
    _debounceTimer?.cancel();
    _saveNow();
    _controller.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    try {
      final doc = await DocumentService.getById(widget.documentId);
      if (!mounted) return;
      setState(() {
        _doc = doc;
        final plainText = _htmlToPlainText(doc.content);
        _controller.text = plainText;
        _controller.selection = TextSelection.collapsed(offset: plainText.length);
        _loading = false;
        _modified = false;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _error = e.toString();
        _loading = false;
      });
    }
  }

  void _onTextChanged() {
    if (!_loading && _doc != null) {
      _modified = true;
      _debounceSave();
    }
  }

  void _debounceSave() {
    _debounceTimer?.cancel();
    _debounceTimer = Timer(const Duration(milliseconds: 1500), () {
      _saveNow();
    });
  }

  Future<void> _saveNow() async {
    if (!_modified || _doc == null) return;
    try {
      final htmlContent = _plainTextToHtml(_controller.text);
      await DocumentService.updateContent(widget.documentId, htmlContent);
      _modified = false;
      ref.read(documentRefreshProvider.notifier).state++;
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('保存失败: $e')),
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    if (_loading) {
      return Scaffold(
        appBar: AppBar(),
        body: const Center(child: CircularProgressIndicator()),
      );
    }

    if (_error != null) {
      return Scaffold(
        appBar: AppBar(),
        body: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(Icons.error_outline, size: 64, color: theme.colorScheme.error),
              const SizedBox(height: 16),
              Text(_error!),
              const SizedBox(height: 16),
              FilledButton(onPressed: _load, child: const Text('重试')),
            ],
          ),
        ),
      );
    }

    final doc = _doc!;
    final updated = _formatDate(doc.updateTime);

    return Scaffold(
      appBar: AppBar(
        title: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              doc.title,
              style: theme.textTheme.titleSmall,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
            if (updated.isNotEmpty)
              Text(
                updated,
                style: theme.textTheme.labelSmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
          ],
        ),
        actions: [
          if (_modified)
            Padding(
              padding: const EdgeInsets.only(right: 8),
              child: Icon(
                Icons.cloud_outlined,
                size: 18,
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          PopupMenuButton<String>(
            onSelected: (action) {
              if (action == 'preview') {
                _showPreview(context);
              }
            },
            itemBuilder: (_) => [
              const PopupMenuItem(value: 'preview', child: Text('预览')),
            ],
          ),
        ],
      ),
      body: Column(
        children: [
          // Content header
          Container(
            width: double.infinity,
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
            decoration: BoxDecoration(
              color: theme.colorScheme.surfaceContainerHighest.withOpacity(0.3),
              border: Border(
                bottom: BorderSide(color: theme.colorScheme.outlineVariant.withOpacity(0.3)),
              ),
            ),
            child: Text(
              '提示：输入纯文本 / Markdown，自动保存',
              style: theme.textTheme.labelSmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant.withOpacity(0.6),
              ),
            ),
          ),
          // Editor
          Expanded(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: TextField(
                controller: _controller,
                maxLines: null,
                expands: true,
                textAlignVertical: TextAlignVertical.top,
                decoration: const InputDecoration(
                  border: InputBorder.none,
                  hintText: '开始书写…',
                  contentPadding: EdgeInsets.zero,
                ),
                style: theme.textTheme.bodyLarge?.copyWith(
                  height: 1.7,
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

  void _showPreview(BuildContext context) {
    _saveNow();
    final html = _plainTextToHtml(_controller.text);
    showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      builder: (ctx) => DraggableScrollableSheet(
        initialChildSize: 0.9,
        minChildSize: 0.5,
        maxChildSize: 0.95,
        expand: false,
        builder: (_, scrollCtrl) => Column(
          children: [
            AppBar(
              title: const Text('预览'),
              leading: IconButton(
                icon: const Icon(Icons.close),
                onPressed: () => Navigator.of(ctx).pop(),
              ),
            ),
            Expanded(
              child: Markdown(
                controller: scrollCtrl,
                data: _htmlToPlainText(html),
              ),
            ),
          ],
        ),
      ),
    );
  }

  /// HTML → 纯文本（仅显示用，编辑器存储纯文本来简化编辑流程）
  String _htmlToPlainText(String html) {
    if (html.isEmpty) return '';
    try {
      final reg = RegExp(r'<[^>]*>', multiLine: true);
      return html.replaceAll(reg, '\n').replaceAll(RegExp(r'\n{3,}'), '\n\n').trim();
    } catch (_) {
      return html;
    }
  }

  /// 纯文本 → HTML（与桌面端兼容）
  String _plainTextToHtml(String text) {
    if (text.isEmpty) return '';
    // Simple markdown → HTML conversion for storage compatibility
    // Full markdown parsing would use a proper converter;
    // this stores as HTML for desktop compatibility
    final lines = text.split('\n');
    final result = StringBuffer();
    result.writeln('<p>');
    for (final line in lines) {
      if (line.trim().isEmpty) {
        result.writeln('</p><p>');
      } else {
        result.writeln('${line.trim()}<br>');
      }
    }
    result.writeln('</p>');
    return result.toString();
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
