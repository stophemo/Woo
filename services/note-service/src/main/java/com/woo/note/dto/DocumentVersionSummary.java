package com.woo.note.dto;

import lombok.Data;

import java.time.LocalDateTime;

/**
 * 版本列表项（不含正文内容，避免传输过大）
 */
@Data
public class DocumentVersionSummary {
    private Long id;
    private Long documentId;
    private Integer versionNo;
    private String title;
    private String changeType;
    private Long operatorId;
    private LocalDateTime createTime;
    /** 内容预览（纯文本，截断后用于列表展示） */
    private String preview;
}
