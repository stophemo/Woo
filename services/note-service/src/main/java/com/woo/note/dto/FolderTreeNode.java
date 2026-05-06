package com.woo.note.dto;

import lombok.Data;

import java.util.List;

/**
 * 目录树节点（响应）
 */
@Data
public class FolderTreeNode {

    private Long id;
    private Long parentId;
    private String name;
    private Integer sortOrder;
    private List<FolderTreeNode> children;
}
