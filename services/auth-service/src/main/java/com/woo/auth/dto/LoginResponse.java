package com.woo.auth.dto;

import lombok.Builder;
import lombok.Data;

/**
 * 登录响应（Token 信息）
 */
@Data
@Builder
public class LoginResponse {

    private String accessToken;
    private String tokenType;
    private Long expiresIn;
    private Long userId;
    private String username;
    private String nickname;
}
