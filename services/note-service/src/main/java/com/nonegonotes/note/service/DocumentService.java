package com.nonegonotes.note.service;

import com.baomidou.mybatisplus.core.conditions.query.LambdaQueryWrapper;
import com.nonegonotes.common.entity.Document;
import com.nonegonotes.common.entity.Folder;
import com.nonegonotes.common.exception.BusinessException;
import com.nonegonotes.note.dto.DocumentRequest;
import com.nonegonotes.note.mapper.DocumentMapper;
import com.nonegonotes.note.mapper.FolderMapper;
import lombok.RequiredArgsConstructor;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.List;

@Service
@RequiredArgsConstructor
public class DocumentService {

    private final DocumentMapper documentMapper;
    private final FolderMapper folderMapper;
    private final DocumentVersionService documentVersionService;

    /**
     * 获取指定目录下的文稿列表（含内容，用于缩略图展示首行）
     */
    public List<Document> getDocumentsByFolder(Long userId, Long folderId) {
        return documentMapper.selectList(
                new LambdaQueryWrapper<Document>()
                        .eq(Document::getUserId, userId)
                        .eq(Document::getFolderId, folderId)
                        .orderByDesc(Document::getUpdateTime)
        );
    }

    /**
     * 获取文稿详情（含内容）
     */
    public Document getDocument(Long userId, Long documentId) {
        return getAndVerifyOwnership(userId, documentId);
    }

    /**
     * 更新文稿内容，同时根据内容首行同步更新 title。
     * 注：本方法不再自动写入版本快照；版本触发统一由前端根据编辑行为
     * （失焦 / 100字或±10行 / 停顿±3s）调用 saveVersionCommit 完成。
     */
    @Transactional
    public void updateDocumentContent(Long userId, Long documentId, String content) {
        Document doc = getAndVerifyOwnership(userId, documentId);
        doc.setContent(content);
        doc.setTitle(extractFirstLineAsTitle(content));
        documentMapper.updateById(doc);
    }

    /**
     * 前端明确触发的一次“提交版本”：读当前文稿的最新内容，记入快照，并触发历史合并。
     */
    @Transactional
    public void saveVersionCommit(Long userId, Long documentId, String changeType) {
        Document doc = getAndVerifyOwnership(userId, documentId);
        documentVersionService.saveAndCompact(doc, changeType, userId);
    }

    /**
     * 手动保存版本（用户点击“保存为版本”时触发），会强制追加一条 manual 快照。
     */
    @Transactional
    public void saveManualVersion(Long userId, Long documentId) {
        Document doc = getAndVerifyOwnership(userId, documentId);
        documentVersionService.saveAndCompact(doc, "manual", userId);
    }

    /**
     * 从 HTML/纯文本 content 中提取首行纯文本作为标题。
     * - 去除所有标签，常见实体 &nbsp; 视为空格
     * - 按换行切分，取第一行非空文本、截断到 40 字
     * - 内容为空时返回 “新文稿”
     */
    private String extractFirstLineAsTitle(String content) {
        if (content == null || content.isEmpty()) {
            return "新文稿";
        }
        String plain = content
                .replaceAll("<\\s*br\\s*/?\\s*>", "\n")
                .replaceAll("</\\s*(p|div|h[1-6]|li|blockquote|pre)\\s*>", "\n")
                .replaceAll("<[^>]+>", "")
                .replace("&nbsp;", " ")
                .replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"");
        String firstLine = "";
        for (String line : plain.split("\\r?\\n")) {
            String trimmed = line.trim();
            if (!trimmed.isEmpty()) {
                firstLine = trimmed;
                break;
            }
        }
        if (firstLine.isEmpty()) {
            return "新文稿";
        }
        if (firstLine.length() > 40) {
            firstLine = firstLine.substring(0, 40) + "…";
        }
        return firstLine;
    }

    /**
     * 创建文稿
     */
    public Document createDocument(Long userId, DocumentRequest request) {
        // 验证目录归属
        Folder folder = folderMapper.selectById(request.getFolderId());
        if (folder == null || !folder.getUserId().equals(userId)) {
            throw new BusinessException("目录不存在");
        }

        // 生成 Git 分支名
        String branchName = generateBranchName(userId, folder, request.getTitle());

        Document document = new Document();
        document.setUserId(userId);
        document.setFolderId(request.getFolderId());
        document.setTitle(request.getTitle());
        document.setContent("");
        document.setBranchName(branchName);
        document.setSortOrder(0);
        document.setDeleted(0);

        documentMapper.insert(document);
        return document;
    }

    /**
     * 重命名文稿
     */
    public void renameDocument(Long userId, Long documentId, String newTitle) {
        Document doc = getAndVerifyOwnership(userId, documentId);
        doc.setTitle(newTitle);
        documentMapper.updateById(doc);
    }

    /**
     * 删除文稿
     */
    public void deleteDocument(Long userId, Long documentId) {
        getAndVerifyOwnership(userId, documentId);
        documentMapper.deleteById(documentId);
    }

    /**
     * 生成 Git 分支名称：目录名-子目录名-文稿标题
     */
    private String generateBranchName(Long userId, Folder folder, String title) {
        StringBuilder path = new StringBuilder();

        // 递归获取目录路径
        buildFolderPath(folder, path);

        // 拼接文稿标题
        path.append(sanitizeBranchSegment(title));

        return path.toString();
    }

    private void buildFolderPath(Folder folder, StringBuilder path) {
        if (folder.getParentId() != null) {
            Folder parent = folderMapper.selectById(folder.getParentId());
            if (parent != null) {
                buildFolderPath(parent, path);
            }
        }
        path.append(sanitizeBranchSegment(folder.getName())).append("-");
    }

    /**
     * 清理分支名中的非法字符
     */
    private String sanitizeBranchSegment(String segment) {
        return segment.replaceAll("[\\s/\\\\:*?\"<>|]", "_");
    }

    private Document getAndVerifyOwnership(Long userId, Long documentId) {
        Document doc = documentMapper.selectById(documentId);
        if (doc == null || !doc.getUserId().equals(userId)) {
            throw new BusinessException("文稿不存在");
        }
        return doc;
    }
}
