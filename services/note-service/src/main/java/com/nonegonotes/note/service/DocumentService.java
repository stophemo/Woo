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

import java.util.List;

@Service
@RequiredArgsConstructor
public class DocumentService {

    private final DocumentMapper documentMapper;
    private final FolderMapper folderMapper;

    /**
     * 获取指定目录下的文稿列表（不含内容）
     */
    public List<Document> getDocumentsByFolder(Long userId, Long folderId) {
        return documentMapper.selectList(
                new LambdaQueryWrapper<Document>()
                        .select(Document.class, info -> !"content".equals(info.getColumn()))
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
     * 更新文稿内容
     */
    public void updateDocumentContent(Long userId, Long documentId, String content) {
        Document doc = getAndVerifyOwnership(userId, documentId);
        doc.setContent(content);
        documentMapper.updateById(doc);
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
