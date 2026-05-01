package com.nonegonotes.auth.dto;

import lombok.Builder;
import lombok.Data;

/**
 * 当前用户信息响应
 */
@Data
@Builder
public class UserInfoResponse {

    private Long userId;
    private String username;
    private String nickname;
    private String email;
    private String avatar;
}
