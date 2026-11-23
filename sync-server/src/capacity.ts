import { ApiError } from "./http.ts";

export const VAULT_CAPACITY = Object.freeze({
  // D1 免费档单库约 500 MB；base64 后还有约 4/3 开销，因此把解码后
  // 密文上限压到 300 MiB，避免先撞平台存储错误而不是可读的 507。
  operations: 200_000,
  ciphertextBytes: 300 * 1024 * 1024,
});

export function vaultCapacityReachedError(): ApiError {
  return new ApiError(
    507,
    "VAULT_CAPACITY_REACHED",
    "同步空间的密文历史容量已满，需要压缩历史记录或迁移存储后再同步",
    {
      maxOperations: VAULT_CAPACITY.operations,
      maxCiphertextBytes: VAULT_CAPACITY.ciphertextBytes,
    },
  );
}
