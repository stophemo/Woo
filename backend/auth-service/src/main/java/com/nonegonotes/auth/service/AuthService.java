package com.nonegonotes.auth.service;

import com.baomidou.mybatisplus.core.conditions.query.LambdaQueryWrapper;
import com.nonegonotes.auth.dto.LoginRequest;
import com.nonegonotes.auth.dto.LoginResponse;
import com.nonegonotes.auth.dto.RegisterRequest;
import com.nonegonotes.auth.mapper.UserMapper;
import com.nonegonotes.common.entity.User;
import com.nonegonotes.common.exception.BusinessException;
import com.nonegonotes.common.util.JwtUtil;
import lombok.RequiredArgsConstructor;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.security.crypto.password.PasswordEncoder;
import org.springframework.stereotype.Service;

import java.util.HashMap;
import java.util.Map;

@Service
@RequiredArgsConstructor
public class AuthService {

    private final UserMapper userMapper;
    private final PasswordEncoder passwordEncoder;

    @Value("${jwt.secret}")
    private String jwtSecret;

    @Value("${jwt.expiration:86400000}")
    private Long jwtExpiration;

    /**
     * 用户注册
     */
    public void register(RegisterRequest request) {
        // 检查用户名是否已存在
        Long count = userMapper.selectCount(
                new LambdaQueryWrapper<User>().eq(User::getUsername, request.getUsername())
        );
        if (count > 0) {
            throw new BusinessException("用户名已存在");
        }

        // 创建用户
        User user = new User();
        user.setUsername(request.getUsername());
        user.setPassword(passwordEncoder.encode(request.getPassword()));
        user.setNickname(request.getNickname() != null ? request.getNickname() : request.getUsername());
        user.setEmail(request.getEmail());
        user.setStatus(1);
        user.setDeleted(0);

        userMapper.insert(user);
    }

    /**
     * 用户登录
     */
    public LoginResponse login(LoginRequest request) {
        // 查询用户
        User user = userMapper.selectOne(
                new LambdaQueryWrapper<User>().eq(User::getUsername, request.getUsername())
        );
        if (user == null) {
            throw new BusinessException(401, "用户名或密码错误");
        }

        // 验证密码
        if (!passwordEncoder.matches(request.getPassword(), user.getPassword())) {
            throw new BusinessException(401, "用户名或密码错误");
        }

        // 检查状态
        if (user.getStatus() != 1) {
            throw new BusinessException(403, "账号已被禁用");
        }

        // 生成 Token
        Map<String, Object> claims = new HashMap<>();
        claims.put("userId", user.getId());
        claims.put("username", user.getUsername());

        String token = JwtUtil.generateToken(jwtSecret, jwtExpiration, claims);

        return LoginResponse.builder()
                .accessToken(token)
                .tokenType("Bearer")
                .expiresIn(jwtExpiration / 1000)
                .userId(user.getId())
                .username(user.getUsername())
                .nickname(user.getNickname())
                .build();
    }
}
