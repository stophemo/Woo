package com.woo.note.dto;

import lombok.Data;

/**
 * 更新文稿内容请求
 */
@Data
public class DocumentContentRequest {

    /** 文稿 HTML 内容 */
    private String content;
}
