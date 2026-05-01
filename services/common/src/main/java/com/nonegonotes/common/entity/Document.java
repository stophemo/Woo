package com.nonegonotes.common.entity;

import com.baomidou.mybatisplus.annotation.*;
import lombok.Data;

import java.time.LocalDateTime;

/**
 * 文稿实体（元数据）
 */
@Data
@TableName("note_document")
public class Document {

    @TableId(type = IdType.ASSIGN_ID)
    private Long id;

    /** 所属用户 ID */
    private Long userId;

    /** 所属目录 ID */
    private Long folderId;

    /** 文稿标题 */
    private String title;

    /** 文稿内容（HTML） */
    private String content;

    /** Git 分支名称 */
    private String branchName;

    /** 排序号 */
    private Integer sortOrder;

    @TableField(fill = FieldFill.INSERT)
    private LocalDateTime createTime;

    @TableField(fill = FieldFill.INSERT_UPDATE)
    private LocalDateTime updateTime;

    @TableLogic
    private Integer deleted;
}
