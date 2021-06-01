### 查询当前高度Hash

```bash
curl http://localhost:9933 -H "Content-Type:application/json;charset=utf-8" -d   '{
     "jsonrpc":"2.0",
      "id":1,
      "method":"chain_getBlockHash",
      "params": []
    }'
{"jsonrpc":"2.0","result":"0x987898b4b27051d13ee47f45eefe053c9f09a37393c9a601ac27acb27e6a265e","id":1}
```

### 查询在线奖励系统的信息

```bash
curl http://localhost:9933 -H "Content-Type:application/json;charset=utf-8" -d   '{
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


### 查询共有多少矿工

```json
curl http://localhost:9933 -H "Content-Type:application/json;charset=utf-8" -d   '{
     "jsonrpc":"2.0",
      "id":1,
      "method":"onlineProfile_getStakerNum",
      "params": []
}'
```

###  分页查询矿工质押详情

```json
{
     "jsonrpc":"2.0",
      "id":1,
      "method":"onlineProfile_getStakerListInfo",
      "params": ["0xe83d6e9b7c27c10a280b6544a8a04c81db946dd2fdf9c8fdea499e464c6d8306", 0, 7]
}
```

+ 参数说明：当前高度Hash，cur_page, per_page

返回结果：

```json
{
   "jsonrpc": "2.0",
   "result": [
      {
         "calcPoints": 0,
         "gpuNum": 0,
         "gpuRentRate": 0,
         "stakerAccount": "5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL",
         "stakerName": [],
         "totalReward": "0"
      },
      {
         "calcPoints": 0,
         "gpuNum": 0,
         "gpuRentRate": 0,
         "stakerAccount": "5HpG9w8EBLe5XCrbczpwq5TSXvedjrBGCwqxK1iQ7qUsSWFc",
         "stakerName": [],
         "totalReward": "0"
      },
      {
         "calcPoints": 0,
         "gpuNum": 0,
         "gpuRentRate": 0,
         "stakerAccount": "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty",
         "stakerName": [],
         "totalReward": "0"
      },
      {
         "calcPoints": 0,
         "gpuNum": 0,
         "gpuRentRate": 0,
         "stakerAccount": "5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL",
         "stakerName": [],
         "totalReward": "0"
      },
      {
         "calcPoints": 0,
         "gpuNum": 0,
         "gpuRentRate": 0,
         "stakerAccount": "5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL",
         "stakerName": [],
         "totalReward": "0"
      },
      {
         "calcPoints": 0,
         "gpuNum": 0,
         "gpuRentRate": 0,
         "stakerAccount": "5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL",
         "stakerName": [],
         "totalReward": "0"
      },
      {
         "calcPoints": 0,
         "gpuNum": 0,
         "gpuRentRate": 0,
         "stakerAccount": "5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL",
         "stakerName": [],
         "totalReward": "0"
      }
   ],
   "id": 1
}
```



---

---



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

### 分页查询矿工账户ID
```bash
curl http://localhost:9933 -H "Content-Type:application/json;charset=utf-8" -d   '{
     "jsonrpc":"2.0",
      "id":1,
      "method":"onlineProfile_getStakerList",
      "params": ["0xbd7e44182d643e9da10402ceaa4bcb17c5995550da73bb9187f73081903cb567", 7, 7]
    }'
```

+ 返回信息：
```json
{"jsonrpc":"2.0","result":["5DhR2dxiPZquPhFjfPzFg5jZENdr375hbX643kr9FBXMVa2z", "5FEmxL86rj2av2X1p7bVvLWZx7CSdFDUmhmWMF1EjUeoB9wg", "5Ebn8seCXL3cj2PDpsgTpXAnuvH24RbSgpxnCmKGxcwANFQ8"],"id":1}
```

### 查询地址对应的账户名称
```bash
curl http://localhost:9933 -H "Content-Type:application/json;charset=utf-8" -d   '{
     "jsonrpc":"2.0",
      "id":1,
      "method":"onlineProfile_getStakerIdentity",
      "params": ["0xbc5d40d87d829a76eb987bb388d05e3d848eec7d91009f3efd30de67a229f116", "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty"]
    }'
```

+ 返回信息：
```json
{"jsonrpc":"2.0","result":[98,111,98],"id":1}
```

python 中decode得到账户名称：
```
>>> bytes([98,111,98]).decode('utf-8')
'bob'
```