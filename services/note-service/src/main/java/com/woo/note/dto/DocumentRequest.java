package com.woo.note.dto;

import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import lombok.Data;

/**
 * 创建/更新文稿请求
 */
@Data
public class DocumentRequest {

    @NotBlank(message = "文稿标题不能为空")
    private String title;

    @NotNull(message = "所属目录不能为空")
    private Long folderId;
}
