package com.woo.note.controller;

import com.woo.common.result.R;
import com.woo.note.dto.FolderRequest;
import com.woo.note.dto.FolderTreeNode;
import com.woo.note.service.FolderService;
import jakarta.validation.Valid;
import lombok.RequiredArgsConstructor;
import org.springframework.web.bind.annotation.*;

import java.util.List;

@RestController
@RequestMapping("/api/folders")
@RequiredArgsConstructor
public class FolderController {

    private final FolderService folderService;

    @GetMapping
    public R<List<FolderTreeNode>> getFolderTree(@RequestHeader("X-User-Id") Long userId) {
        List<FolderTreeNode> tree = folderService.getFolderTree(userId);
        return R.ok(tree);
    }

    @PostMapping
    public R<Long> createFolder(@RequestHeader("X-User-Id") Long userId,
                                @Valid @RequestBody FolderRequest request) {
        Long folderId = folderService.createFolder(userId, request);
        return R.ok(folderId);
    }

    @PutMapping("/{folderId}/rename")
    public R<Void> renameFolder(@RequestHeader("X-User-Id") Long userId,
                                @PathVariable Long folderId,
                                @RequestParam String name) {
        folderService.renameFolder(userId, folderId, name);
        return R.ok();
    }

    @DeleteMapping("/{folderId}")
    public R<Void> deleteFolder(@RequestHeader("X-User-Id") Long userId,
                                @PathVariable Long folderId) {
        folderService.deleteFolder(userId, folderId);
        return R.ok();
    }
}
