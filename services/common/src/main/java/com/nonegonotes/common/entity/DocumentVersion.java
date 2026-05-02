package com.nonegonotes.common.entity;

import com.baomidou.mybatisplus.annotation.FieldFill;
import com.baomidou.mybatisplus.annotation.IdType;
import com.baomidou.mybatisplus.annotation.TableField;
import com.baomidou.mybatisplus.annotation.TableId;
import com.baomidou.mybatisplus.annotation.TableName;
import lombok.Data;

import java.time.LocalDateTime;

/**
 * 文稿版本快照实体
 */
@Data
@TableName("note_document_version")
public class DocumentVersion {

    @TableId(type = IdType.ASSIGN_ID)
    private Long id;

    /** 文稿 ID */
    private Long documentId;

    /** 所属用户 ID（冗余字段，便于鉴权） */
    private Long userId;

    /** 版本号（按文稿从 1 递增） */
    private Integer versionNo;

    /** 版本标题快照 */
    private String title;

    /** 版本内容快照（HTML 全量） */
    private String content;

    /** 内容 SHA-256，用于去重 */
    private String contentHash;

    /** 变更类型：auto / manual / restore */
    private String changeType;

    /** 操作人 ID */
    private Long operatorId;

    @TableField(fill = FieldFill.INSERT)
    private LocalDateTime createTime;
}
