package com.nonegonotes.common.entity;

import com.baomidou.mybatisplus.annotation.*;
import lombok.Data;

import java.time.LocalDateTime;

/**
 * 目录实体
 */
@Data
@TableName("note_folder")
public class Folder {

    @TableId(type = IdType.ASSIGN_ID)
    private Long id;

    /** 所属用户 ID */
    private Long userId;

    /** 父目录 ID，顶级目录为 null */
    private Long parentId;

    /** 目录名称 */
    private String name;

    /** 排序号 */
    private Integer sortOrder;

    @TableField(fill = FieldFill.INSERT)
    private LocalDateTime createTime;

    @TableField(fill = FieldFill.INSERT_UPDATE)
    private LocalDateTime updateTime;

    @TableLogic
    private Integer deleted;
}
