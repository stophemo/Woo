USE `woo_notes`;

CREATE TABLE IF NOT EXISTS `note_document_version` (
    `id` BIGINT NOT NULL COMMENT '版本ID',
    `document_id` BIGINT NOT NULL COMMENT '文稿ID',
    `user_id` BIGINT NOT NULL COMMENT '所属用户ID（冗余，便于鉴权/统计）',
    `version_no` INT NOT NULL COMMENT '版本号（从1递增，按文稿隔离）',
    `title` VARCHAR(200) DEFAULT NULL COMMENT '版本标题快照',
    `content` LONGTEXT COMMENT '版本内容快照（HTML 全量）',
    `content_hash` CHAR(64) DEFAULT NULL COMMENT '内容SHA-256，用于去重',
    `change_type` VARCHAR(16) NOT NULL DEFAULT 'auto' COMMENT '变更类型：auto-自动保存 / manual-手动保存 / restore-回滚生成',
    `operator_id` BIGINT DEFAULT NULL COMMENT '操作人ID',
    `create_time` DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_document_version` (`document_id`, `version_no`),
    KEY `idx_document_id` (`document_id`),
    KEY `idx_user_id` (`user_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='文稿版本快照表';

SHOW TABLES LIKE 'note_document_version';
DESC note_document_version;
