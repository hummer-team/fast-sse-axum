## License
This project is licensed under the [Apache License 2.0](LICENSE).

# Fast Sse
This is a compact, high-performance SSE Server. base on axum
## Features
1. Broadcast message
2. Compressed message
3. Dead letter queue, storing messages that failed to be delivered
4. Redis Stream as Queue
5. Support for request signature authentication
6. **Kafka as a message storage implementation**
7. **Send indicator monitoring, implementation in progress**
# Base test report
## Test parameters
1. producers 2 vus send 50KB body message
2. consumer 50 vus
3. details
```js
let scenarios = {
    consumers: {
        executor: 'ramping-vus',
        exec: 'consumers',
        stages: [
            { duration: '15s', target: 30 },
            // vus add count 20~50
            { duration: '45s', target: MAX_CONSUMER_VUS },
            { duration: '45s', target: MAX_CONSUMER_VUS },
            { duration: '30s', target: 0 },
        ],
    },
    producers: {
        // executor: 
        // per-vu-iterations, per vus execute count
        // shared-iterations, all vus execute count
        // constant-vus
        executor: 'constant-vus',
        exec: 'producers',
        vus: 2,
        // use shared-iterations then use: maxDuration and iterations
        // maxDuration: '4m',
        // iterations: 10,
        "duration": '1m15s',
        // Delay the start by 10 seconds to allow consumers to establish a connection first
        startTime: '50s',
    },
};
```
## Test result
 ```txt
█ THRESHOLDS 
    decompress_latency_ms{scenario:consumers}
    ✓ 'p(95)<1000' p(95)=37ms

    http_req_duration{scenario:producers}
    ✓ 'p(95)<500' p(95)=23.1ms

    http_req_failed{scenario:producers}
    ✓ 'rate<0.01' rate=0.00%

    message_latency_ms{scenario:consumers}
    ✓ 'p(95)<1000' p(95)=7ms

    produced_message_count{scenario:producers}
    ✓ 'count > 0' count=148

    produced_message_size_bytes{scenario:producers}
    ✓ 'count > 0' count=7577600

    received_message_count{scenario:consumers}
    ✓ 'count > 0' count=141

    received_message_size_bytes{scenario:consumers}
    ✓ 'count > 0' count=83512

    sse_connection_ok{scenario:consumers}
    ✓ 'rate>0.99' rate=0.00%


█ TOTAL RESULTS 

    checks_total.......: 437     2.648456/s
    checks_succeeded...: 100.00% 437 out of 437
    checks_failed......: 0.00%   0 out of 437

    ✓ producer POST status is 200
    ✓ producer response contains message_id
    ✓ event data is not empty

█ CUSTOM
    decompress_latency_ms..........: avg=27.95ms min=24ms   med=26ms    max=39ms    p(90)=36ms    p(95)=37ms  
      { scenario:consumers }.......: avg=27.95ms min=24ms   med=26ms    max=39ms    p(90)=36ms    p(95)=37ms  
    message_latency_ms.............: avg=3.82ms  min=2ms    med=3ms     max=8ms     p(90)=6ms     p(95)=7ms   
      { scenario:consumers }.......: avg=3.82ms  min=2ms    med=3ms     max=8ms     p(90)=6ms     p(95)=7ms   
    produced_message_count.........: 148      0.89696/s
      { scenario:producers }.......: 148      0.89696/s
    produced_message_size_bytes....: 7577600  45924.341925/s
      { scenario:producers }.......: 7577600  45924.341925/s
    received_message_count.........: 141      0.854536/s
      { scenario:consumers }.......: 141      0.854536/s
    received_message_size_bytes....: 83512    506.127751/s
      { scenario:consumers }.......: 83512    506.127751/s
    sse_connection_ok..............: 0.00%    0 out of 0
      { scenario:consumers }.......: 0.00%    0 out of 0
    sse_event......................: 142      0.860597/s

█ HTTP
    http_req_duration..............: avg=17.02ms min=6.01ms med=17.63ms max=45.41ms p(90)=20.62ms p(95)=23.1ms
      { expected_response:true }...: avg=17.02ms min=6.01ms med=17.63ms max=45.41ms p(90)=20.62ms p(95)=23.1ms
      { scenario:producers }.......: avg=17.02ms min=6.01ms med=17.63ms max=45.41ms p(90)=20.62ms p(95)=23.1ms
    http_req_failed................: 0.00%    0 out of 148
      { scenario:producers }.......: 0.00%    0 out of 148
    http_reqs......................: 148      0.89696/s

█ EXECUTION
    iteration_duration.............: avg=4.17µs  min=250ns  med=459ns   max=1.04s   p(90)=791ns   p(95)=958ns 
    iterations.....................: 41569956 251936.348338/s
    vus............................: 1        min=1           max=52
    vus_max........................: 52       min=52          max=52

    NETWORK
    data_received..................: 119 kB   723 B/s
    data_sent......................: 37 kB    227 B/s

running (2m45.0s), 00/52 VUs, 41569956 complete and 48 interrupted iterations
consumers ✓ [======================================] 01/50 VUs  2m15s
producers ✓ [======================================] 2 VUs      1m15s

█ Summary
================================================================
                      SSE Message Loss Report
----------------------------------------------------------------
  Messages Produced : 148
  Messages Received : 141
  Messages Lost     : 7
  Message Loss Rate : 4.73%
================================================================
```
### Message Loss Rate
This is attributable to the K6 load testing strategy, as consumers require an established connection before producers can send messages, enabling consumers to receive messages in full. K6's support for this scenario is incomplete, and this does not constitute a bug in the current SSE implementation.
# Use step
1. build source code
2. build redis server

## Postman test
1. signature eventId and eventType(`use hmac algorithm`)
2. subscribe
```bash
curl --location 'http://localhost:30000/v1/subscribe/events/carid000001/types/cart_changed' \
--header 'x-signature: xx'
```
3. publish
```bash
curl --location 'http://localhost:30000/v1/sse/events/carid000001/types/cart_changed' \
--header 'x-signature: xx' \
--header 'Content-Type: application/json' \
--data '{
    "event_name": "CartItemUpdatedssss",
    "user": {
        "from_user": "system-inventory",
        "to_user": "user-session-abc123"
    },
    "data": {
        "car_id": "carid00000177777",
        "event_type": "cart_changed",
        "timestamp": 1234567890,
        "items": [
            {
                "sku": "SKU-45678",
                "quantity": 2,
                "price": 299.99,
                "action": "added"
            }
        ],
        "total_items": 2,
        "total_amount": 599.98
    },
    "headers": {
        "X-Request-ID": "req-uuid-1234666677",
        "X-Message-Priority": "high",
        "X-Trace-ID": "trace-abc-def"
    }
}'
```