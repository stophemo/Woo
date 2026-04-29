package com.nonegonotes.note.service;

import com.baomidou.mybatisplus.core.conditions.query.LambdaQueryWrapper;
import com.nonegonotes.common.entity.Folder;
import com.nonegonotes.common.exception.BusinessException;
import com.nonegonotes.note.dto.FolderRequest;
import com.nonegonotes.note.dto.FolderTreeNode;
import com.nonegonotes.note.mapper.FolderMapper;
import lombok.RequiredArgsConstructor;
import org.springframework.stereotype.Service;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;

@Service
@RequiredArgsConstructor
public class FolderService {

    private final FolderMapper folderMapper;

    /**
     * 获取用户目录树
     */
    public List<FolderTreeNode> getFolderTree(Long userId) {
        List<Folder> folders = folderMapper.selectList(
                new LambdaQueryWrapper<Folder>()
                        .eq(Folder::getUserId, userId)
                        .orderByAsc(Folder::getSortOrder)
        );
        return buildTree(folders);
    }

    /**
     * 创建目录
     */
    public Long createFolder(Long userId, FolderRequest request) {
        Folder folder = new Folder();
        folder.setUserId(userId);
        folder.setName(request.getName());
        folder.setParentId(request.getParentId());
        folder.setSortOrder(0);
        folder.setDeleted(0);

        folderMapper.insert(folder);
        return folder.getId();
    }

    /**
     * 重命名目录
     */
    public void renameFolder(Long userId, Long folderId, String newName) {
        Folder folder = getAndVerifyOwnership(userId, folderId);
        folder.setName(newName);
        folderMapper.updateById(folder);
    }

    /**
     * 删除目录（级联删除子目录）
     */
    public void deleteFolder(Long userId, Long folderId) {
        getAndVerifyOwnership(userId, folderId);
        // 删除自身及所有子目录
        deleteRecursive(userId, folderId);
    }

    private void deleteRecursive(Long userId, Long folderId) {
        // 查找所有子目录
        List<Folder> children = folderMapper.selectList(
                new LambdaQueryWrapper<Folder>()
                        .eq(Folder::getUserId, userId)
                        .eq(Folder::getParentId, folderId)
        );
        for (Folder child : children) {
            deleteRecursive(userId, child.getId());
        }
        folderMapper.deleteById(folderId);
    }

    private Folder getAndVerifyOwnership(Long userId, Long folderId) {
        Folder folder = folderMapper.selectById(folderId);
        if (folder == null || !folder.getUserId().equals(userId)) {
            throw new BusinessException("目录不存在");
        }
        return folder;
    }

    /**
     * 构建目录树
     */
    private List<FolderTreeNode> buildTree(List<Folder> folders) {
        Map<Long, List<Folder>> parentMap = folders.stream()
                .collect(Collectors.groupingBy(f -> f.getParentId() == null ? 0L : f.getParentId()));

        return buildChildren(parentMap, 0L);
    }

    private List<FolderTreeNode> buildChildren(Map<Long, List<Folder>> parentMap, Long parentId) {
        List<Folder> children = parentMap.getOrDefault(parentId, new ArrayList<>());
        return children.stream().map(folder -> {
            FolderTreeNode node = new FolderTreeNode();
            node.setId(folder.getId());
            node.setParentId(folder.getParentId());
            node.setName(folder.getName());
            node.setSortOrder(folder.getSortOrder());
            node.setChildren(buildChildren(parentMap, folder.getId()));
            return node;
        }).collect(Collectors.toList());
    }
}
