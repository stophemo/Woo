package com.nonegonotes.note.controller;

import com.nonegonotes.common.entity.Document;
import com.nonegonotes.common.result.R;
import com.nonegonotes.note.dto.DocumentContentRequest;
import com.nonegonotes.note.dto.DocumentRequest;
import com.nonegonotes.note.service.DocumentService;
import jakarta.validation.Valid;
import lombok.RequiredArgsConstructor;
import org.springframework.web.bind.annotation.*;

import java.util.List;

@RestController
@RequestMapping("/api/documents")
@RequiredArgsConstructor
public class DocumentController {

    private final DocumentService documentService;

    @GetMapping
    public R<List<Document>> getDocumentsByFolder(@RequestHeader("X-User-Id") Long userId,
                                                   @RequestParam Long folderId) {
        List<Document> documents = documentService.getDocumentsByFolder(userId, folderId);
        return R.ok(documents);
    }

    @GetMapping("/{documentId}")
    public R<Document> getDocument(@RequestHeader("X-User-Id") Long userId,
                                    @PathVariable Long documentId) {
        Document document = documentService.getDocument(userId, documentId);
        return R.ok(document);
    }

    @PostMapping
    public R<Document> createDocument(@RequestHeader("X-User-Id") Long userId,
                                      @Valid @RequestBody DocumentRequest request) {
        Document document = documentService.createDocument(userId, request);
        return R.ok(document);
    }

    @PutMapping("/{documentId}/rename")
    public R<Void> renameDocument(@RequestHeader("X-User-Id") Long userId,
                                  @PathVariable Long documentId,
                                  @RequestParam String title) {
        documentService.renameDocument(userId, documentId, title);
        return R.ok();
    }

    @PutMapping("/{documentId}/content")
    public R<Void> updateContent(@RequestHeader("X-User-Id") Long userId,
                                  @PathVariable Long documentId,
                                  @RequestBody DocumentContentRequest request) {
        documentService.updateDocumentContent(userId, documentId, request.getContent());
        return R.ok();
    }

    @DeleteMapping("/{documentId}")
    public R<Void> deleteDocument(@RequestHeader("X-User-Id") Long userId,
                                  @PathVariable Long documentId) {
        documentService.deleteDocument(userId, documentId);
        return R.ok();
    }
}
