package com.woo.note.controller;

import com.woo.common.entity.DocumentVersion;
import com.woo.common.result.R;
import com.woo.note.dto.DocumentVersionSummary;
import com.woo.note.service.DocumentService;
import com.woo.note.service.DocumentVersionService;
import lombok.RequiredArgsConstructor;
import org.springframework.web.bind.annotation.*;

import java.util.List;

/**
 * 文稿版本接口
 */
@RestController
@RequestMapping("/api/documents/{documentId}/versions")
@RequiredArgsConstructor
public class DocumentVersionController {

    private final DocumentVersionService documentVersionService;
    private final DocumentService documentService;

    /** 获取版本列表（不含正文） */
    @GetMapping
    public R<List<DocumentVersionSummary>> listVersions(@RequestHeader("X-User-Id") Long userId,
                                                        @PathVariable Long documentId) {
        return R.ok(documentVersionService.listVersions(userId, documentId));
    }

    /** 获取某版本的完整内容 */
    @GetMapping("/{versionNo}")
    public R<DocumentVersion> getVersion(@RequestHeader("X-User-Id") Long userId,
                                         @PathVariable Long documentId,
                                         @PathVariable Integer versionNo) {
        return R.ok(documentVersionService.getVersion(userId, documentId, versionNo));
    }

    /** 手动保存当前文稿为一个新版本 */
    @PostMapping
    public R<Void> saveManual(@RequestHeader("X-User-Id") Long userId,
                              @PathVariable Long documentId) {
        documentService.saveManualVersion(userId, documentId);
        return R.ok();
    }

    /**
     * 前端触发的自动版本提交：失焦 / 变更量达阈值 / 停顿 3s 时调用。
     * @param changeType 默认 auto；允许 auto / blur / threshold / idle 等子类型（当前后端只区分 auto/manual/restore，其余都归为 auto）。
     */
    @PostMapping("/commit")
    public R<Void> commit(@RequestHeader("X-User-Id") Long userId,
                          @PathVariable Long documentId,
                          @RequestParam(value = "changeType", required = false, defaultValue = "auto") String changeType) {
        String normalized = "manual".equalsIgnoreCase(changeType) ? "manual" : "auto";
        documentService.saveVersionCommit(userId, documentId, normalized);
        return R.ok();
    }

    /** 回滚到指定版本（写回正文并追加一条 restore 版本） */
    @PostMapping("/{versionNo}/restore")
    public R<DocumentVersion> restore(@RequestHeader("X-User-Id") Long userId,
                                      @PathVariable Long documentId,
                                      @PathVariable Integer versionNo) {
        return R.ok(documentVersionService.restore(userId, documentId, versionNo));
    }
}
