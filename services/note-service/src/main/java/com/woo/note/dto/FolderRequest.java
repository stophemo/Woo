package com.woo.note.dto;

import jakarta.validation.constraints.NotBlank;
import lombok.Data;

/**
 * 创建/更新目录请求
 */
@Data
public class FolderRequest {

    @NotBlank(message = "目录名称不能为空")
    private String name;

    /** 父目录 ID，顶级目录为 null */
    private Long parentId;
}
