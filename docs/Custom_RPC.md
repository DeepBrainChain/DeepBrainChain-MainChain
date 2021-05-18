### 查询矿工的质押数量

```bash
curl http://localhost:9933 -H "Content-Type:application/json;charset=utf-8" -d   '{
     "jsonrpc":"2.0",
      "id":1,
      "method":"onlineProfile_getStakerInfo",
      "params": ["0x1a6cf8f12ea4d3ac5c4ac2d8a0e91a08fc1e416917e4f16a5328bd775a0f1919","5GjrZ4iQdxZhAKKjNooMruGqBwH5CwbJ6Un6Cinc7j45zToE"]
    }'
```

+ "params"为：["Block_hash", "AccountId"]

+ 返回信息：

  ```json
  {"jsonrpc":"2.0","result":{"calcPoints":0,"gpuNum":0,"totalReward":"0"},"id":1}
  ```

### 查询在线奖励系统的信息

```bash
❯ curl http://localhost:9933 -H "Content-Type:application/json;charset=utf-8" -d   '{
     "jsonrpc":"2.0",
      "id":1,
      "method":"onlineProfile_getOpInfo",
      "params": []
    }'
```

+ 返回信息：

  ```json
  {"jsonrpc":"2.0","result":{"totalCalcPoints":0,"totalGpuNum":0,"totalStake":"0","totalStaker":0},"id":1}
  ```

  