package com.nonegonotes.note.service;

import com.baomidou.mybatisplus.core.conditions.query.LambdaQueryWrapper;
import com.nonegonotes.common.entity.Document;
import com.nonegonotes.common.entity.DocumentVersion;
import com.nonegonotes.common.exception.BusinessException;
import com.nonegonotes.note.dto.DocumentVersionSummary;
import com.nonegonotes.note.mapper.DocumentMapper;
import com.nonegonotes.note.mapper.DocumentVersionMapper;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.time.LocalDateTime;
import java.util.ArrayList;
import java.util.List;

/**
 * 文稿版本服务：快照写入、历史合并、列表查询、内容获取、回滚。
 *
 * <p>版本触发策略（由前端驱动）：</p>
 * <ul>
 *   <li>编辑器失焦（blur）</li>
 *   <li>自上次版本起内容变更 ≥ 100 字 或 ≥ 10 行</li>
 *   <li>编辑停顿 ≥ 3 秒</li>
 * </ul>
 *
 * <p>历史合并策略（每次新增版本后执行）：</p>
 * <ul>
 *   <li>最近 24 小时：全部保留</li>
 *   <li>24 小时 ~ 1 周：相邻版本 edit distance ≤ 100 则合并（删除较旧的那条）</li>
 *   <li>超过 1 周：相邻版本 edit distance ≤ 1000 则合并</li>
 * </ul>
 */
@Slf4j
@Service
@RequiredArgsConstructor
public class DocumentVersionService {

    private final DocumentVersionMapper versionMapper;
    private final DocumentMapper documentMapper;

    private static final int PREVIEW_LIMIT = 80;
    /** 最近窗口：不合并 */
    private static final long RECENT_HOURS = 24;
    /** 中期窗口：≤ 100 字变化视为相似 */
    private static final long MID_DAYS = 7;
    private static final int MID_DIFF_THRESHOLD = 100;
    /** 远期窗口：≤ 1000 字变化视为相似 */
    private static final int OLD_DIFF_THRESHOLD = 1000;
    /** edit distance 的规模上限；超出则退化为长度差比较 */
    private static final int EDIT_DISTANCE_MAX_LEN = 10_000;

    // ======================================================================
    // 写入
    // ======================================================================

    /**
     * 写入一条快照并触发历史合并。
     * 若内容与最新版本完全一致（hash 相同）则跳过，返回 null。
     */
    @Transactional
    public DocumentVersion saveAndCompact(Document document, String changeType, Long operatorId) {
        DocumentVersion saved = saveSnapshot(document, changeType, operatorId);
        if (saved != null) {
            compactHistory(document.getId());
        }
        return saved;
    }

    /**
     * 写入一条快照（hash 去重，不做时间节流）。
     */
    @Transactional
    public DocumentVersion saveSnapshot(Document document, String changeType, Long operatorId) {
        if (document == null) {
            return null;
        }
        String content = document.getContent() == null ? "" : document.getContent();
        String hash = sha256(content);

        DocumentVersion latest = findLatest(document.getId());
        if (latest != null && hash.equals(latest.getContentHash())) {
            return null;
        }

        int nextVersionNo = (latest == null ? 0 : latest.getVersionNo()) + 1;
        DocumentVersion version = new DocumentVersion();
        version.setDocumentId(document.getId());
        version.setUserId(document.getUserId());
        version.setVersionNo(nextVersionNo);
        version.setTitle(document.getTitle());
        version.setContent(content);
        version.setContentHash(hash);
        version.setChangeType(changeType == null ? "auto" : changeType);
        version.setOperatorId(operatorId);
        versionMapper.insert(version);
        return version;
    }

    // ======================================================================
    // 历史合并
    // ======================================================================

    /**
     * 按时间分层合并旧版本：
     *  - 24h 内：全保留
     *  - 24h ~ 1w：相邻差 ≤ 100 字 → 删除较旧的
     *  - 超过 1w：相邻差 ≤ 1000 字 → 删除较旧的
     */
    @Transactional
    public void compactHistory(Long documentId) {
        List<DocumentVersion> versions = versionMapper.selectList(
                new LambdaQueryWrapper<DocumentVersion>()
                        .eq(DocumentVersion::getDocumentId, documentId)
                        .orderByAsc(DocumentVersion::getVersionNo)
        );
        if (versions.size() < 2) return;

        LocalDateTime now = LocalDateTime.now();
        DocumentVersion prev = null;
        for (DocumentVersion curr : versions) {
            if (prev == null) {
                prev = curr;
                continue;
            }
            int threshold = thresholdFor(prev.getCreateTime(), now);
            if (threshold > 0) {
                int diff = textDiff(prev.getContent(), curr.getContent());
                if (diff <= threshold) {
                    versionMapper.deleteById(prev.getId());
                    prev = curr;
                    continue;
                }
            }
            prev = curr;
        }
    }

    /**
     * 返回给定版本时间应当适用的"相邻差阈值"；0 表示不合并。
     */
    private int thresholdFor(LocalDateTime createTime, LocalDateTime now) {
        if (createTime == null) return 0;
        if (createTime.isAfter(now.minusHours(RECENT_HOURS))) {
            return 0;
        }
        if (createTime.isAfter(now.minusDays(MID_DAYS))) {
            return MID_DIFF_THRESHOLD;
        }
        return OLD_DIFF_THRESHOLD;
    }

    // ======================================================================
    // 查询
    // ======================================================================

    public List<DocumentVersionSummary> listVersions(Long userId, Long documentId) {
        verifyOwnership(userId, documentId);
        List<DocumentVersion> list = versionMapper.selectList(
                new LambdaQueryWrapper<DocumentVersion>()
                        .eq(DocumentVersion::getDocumentId, documentId)
                        .orderByDesc(DocumentVersion::getVersionNo)
        );
        List<DocumentVersionSummary> result = new ArrayList<>(list.size());
        for (DocumentVersion v : list) {
            DocumentVersionSummary s = new DocumentVersionSummary();
            s.setId(v.getId());
            s.setDocumentId(v.getDocumentId());
            s.setVersionNo(v.getVersionNo());
            s.setTitle(v.getTitle());
            s.setChangeType(v.getChangeType());
            s.setOperatorId(v.getOperatorId());
            s.setCreateTime(v.getCreateTime());
            s.setPreview(buildPreview(v.getContent()));
            result.add(s);
        }
        return result;
    }

    public DocumentVersion getVersion(Long userId, Long documentId, Integer versionNo) {
        verifyOwnership(userId, documentId);
        DocumentVersion v = versionMapper.selectOne(
                new LambdaQueryWrapper<DocumentVersion>()
                        .eq(DocumentVersion::getDocumentId, documentId)
                        .eq(DocumentVersion::getVersionNo, versionNo)
        );
        if (v == null) {
            throw new BusinessException("版本不存在");
        }
        return v;
    }

    /**
     * 回滚：将指定版本内容写回文稿，并生成一条 restore 新版本。
     */
    @Transactional
    public DocumentVersion restore(Long userId, Long documentId, Integer versionNo) {
        Document doc = verifyOwnership(userId, documentId);
        DocumentVersion target = versionMapper.selectOne(
                new LambdaQueryWrapper<DocumentVersion>()
                        .eq(DocumentVersion::getDocumentId, documentId)
                        .eq(DocumentVersion::getVersionNo, versionNo)
        );
        if (target == null) {
            throw new BusinessException("版本不存在");
        }

        doc.setContent(target.getContent());
        doc.setTitle(target.getTitle());
        documentMapper.updateById(doc);

        return saveAndCompact(doc, "restore", userId);
    }

    // ======================================================================
    // 私有工具
    // ======================================================================

    private DocumentVersion findLatest(Long documentId) {
        return versionMapper.selectOne(
                new LambdaQueryWrapper<DocumentVersion>()
                        .eq(DocumentVersion::getDocumentId, documentId)
                        .orderByDesc(DocumentVersion::getVersionNo)
                        .last("LIMIT 1")
        );
    }

    private Document verifyOwnership(Long userId, Long documentId) {
        Document doc = documentMapper.selectById(documentId);
        if (doc == null || !doc.getUserId().equals(userId)) {
            throw new BusinessException("文稿不存在");
        }
        return doc;
    }

    private String sha256(String text) {
        try {
            MessageDigest md = MessageDigest.getInstance("SHA-256");
            byte[] bytes = md.digest(text.getBytes(StandardCharsets.UTF_8));
            StringBuilder sb = new StringBuilder(bytes.length * 2);
            for (byte b : bytes) {
                sb.append(String.format("%02x", b));
            }
            return sb.toString();
        } catch (NoSuchAlgorithmException e) {
            log.error("SHA-256 not available", e);
            return String.valueOf(text.hashCode());
        }
    }

    private String buildPreview(String content) {
        String plain = stripHtml(content);
        if (plain.length() > PREVIEW_LIMIT) {
            return plain.substring(0, PREVIEW_LIMIT) + "…";
        }
        return plain;
    }

    private String stripHtml(String content) {
        if (content == null || content.isEmpty()) return "";
        return content
                .replaceAll("<\\s*br\\s*/?\\s*>", " ")
                .replaceAll("</\\s*(p|div|h[1-6]|li|blockquote|pre)\\s*>", " ")
                .replaceAll("<[^>]+>", "")
                .replace("&nbsp;", " ")
                .replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replaceAll("\\s+", " ")
                .trim();
    }

    /**
     * 两段 HTML 内容的"文本差异量"：剥离 HTML 后的编辑距离。
     * 规模超过 {@link #EDIT_DISTANCE_MAX_LEN} 时退化为长度差，避免 O(n*m) 性能问题。
     */
    private int textDiff(String a, String b) {
        String ta = stripHtml(a);
        String tb = stripHtml(b);
        if (ta.equals(tb)) return 0;
        if (ta.length() > EDIT_DISTANCE_MAX_LEN || tb.length() > EDIT_DISTANCE_MAX_LEN) {
            return Math.abs(ta.length() - tb.length());
        }
        return levenshtein(ta, tb);
    }

    /**
     * Levenshtein 编辑距离，空间优化为 O(min(m,n))。
     */
    private int levenshtein(String a, String b) {
        if (a.length() < b.length()) {
            String t = a; a = b; b = t;
        }
        int m = a.length();
        int n = b.length();
        int[] dp = new int[n + 1];
        for (int j = 0; j <= n; j++) dp[j] = j;
        for (int i = 1; i <= m; i++) {
            int prev = dp[0];
            dp[0] = i;
            for (int j = 1; j <= n; j++) {
                int temp = dp[j];
                int cost = (a.charAt(i - 1) == b.charAt(j - 1)) ? 0 : 1;
                dp[j] = Math.min(
                        Math.min(dp[j] + 1, dp[j - 1] + 1),
                        prev + cost
                );
                prev = temp;
            }
        }
        return dp[n];
    }
}
