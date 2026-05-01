package com.nonegonotes.auth.controller;

import com.nonegonotes.auth.dto.LoginRequest;
import com.nonegonotes.auth.dto.LoginResponse;
import com.nonegonotes.auth.dto.RegisterRequest;
import com.nonegonotes.auth.dto.UserInfoResponse;
import com.nonegonotes.auth.service.AuthService;
import com.nonegonotes.common.result.R;
import jakarta.validation.Valid;
import lombok.RequiredArgsConstructor;
import org.springframework.web.bind.annotation.*;

@RestController
@RequestMapping("/api/auth")
@RequiredArgsConstructor
public class AuthController {

    private final AuthService authService;

    @PostMapping("/register")
    public R<Void> register(@Valid @RequestBody RegisterRequest request) {
        authService.register(request);
        return R.ok();
    }

    @PostMapping("/login")
    public R<LoginResponse> login(@Valid @RequestBody LoginRequest request) {
        LoginResponse response = authService.login(request);
        return R.ok(response);
    }

    @GetMapping("/me")
    public R<UserInfoResponse> me(@RequestHeader("X-User-Id") Long userId) {
        UserInfoResponse info = authService.getUserInfo(userId);
        return R.ok(info);
    }
}
