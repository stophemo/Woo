-- ============================
-- Non-ego Notes Database Schema
-- ============================

CREATE DATABASE IF NOT EXISTS `woo_notes` DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci;

USE `woo_notes`;

-- 用户表
CREATE TABLE IF NOT EXISTS `sys_user` (
    `id` BIGINT NOT NULL COMMENT '用户ID',
    `username` VARCHAR(50) NOT NULL COMMENT '用户名',
    `password` VARCHAR(200) NOT NULL COMMENT '密码（BCrypt加密）',
    `nickname` VARCHAR(50) DEFAULT NULL COMMENT '昵称',
    `email` VARCHAR(100) DEFAULT NULL COMMENT '邮箱',
    `avatar` VARCHAR(500) DEFAULT NULL COMMENT '头像URL',
    `status` TINYINT NOT NULL DEFAULT 1 COMMENT '状态：0-禁用，1-正常',
    `create_time` DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `update_time` DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '更新时间',
    `deleted` TINYINT NOT NULL DEFAULT 0 COMMENT '逻辑删除：0-未删除，1-已删除',
    PRIMARY KEY (`id`),
    UNIQUE KEY `uk_username` (`username`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='用户表';

-- 目录表
CREATE TABLE IF NOT EXISTS `note_folder` (
    `id` BIGINT NOT NULL COMMENT '目录ID',
    `user_id` BIGINT NOT NULL COMMENT '所属用户ID',
    `parent_id` BIGINT DEFAULT NULL COMMENT '父目录ID，顶级为NULL',
    `name` VARCHAR(100) NOT NULL COMMENT '目录名称',
    `sort_order` INT NOT NULL DEFAULT 0 COMMENT '排序号',
    `create_time` DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `update_time` DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '更新时间',
    `deleted` TINYINT NOT NULL DEFAULT 0 COMMENT '逻辑删除：0-未删除，1-已删除',
    PRIMARY KEY (`id`),
    KEY `idx_user_id` (`user_id`),
    KEY `idx_parent_id` (`parent_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='目录表';

-- 文稿表（含内容）
CREATE TABLE IF NOT EXISTS `note_document` (
    `id` BIGINT NOT NULL COMMENT '文稿ID',
    `user_id` BIGINT NOT NULL COMMENT '所属用户ID',
    `folder_id` BIGINT NOT NULL COMMENT '所属目录ID',
    `title` VARCHAR(200) NOT NULL COMMENT '文稿标题',
    `content` LONGTEXT COMMENT '文稿内容（HTML）',
    `branch_name` VARCHAR(300) DEFAULT NULL COMMENT 'Git分支名称',
    `sort_order` INT NOT NULL DEFAULT 0 COMMENT '排序号',
    `create_time` DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP COMMENT '创建时间',
    `update_time` DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT '更新时间',
    `deleted` TINYINT NOT NULL DEFAULT 0 COMMENT '逻辑删除：0-未删除，1-已删除',
    PRIMARY KEY (`id`),
    KEY `idx_user_id` (`user_id`),
    KEY `idx_folder_id` (`folder_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='文稿表';

-- 若表已存在但缺少 content 字段，可手动执行：
-- ALTER TABLE `note_document` ADD COLUMN `content` LONGTEXT COMMENT '文稿内容（HTML）' AFTER `title`;
