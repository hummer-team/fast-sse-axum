import http from 'k6/http';
import { check, sleep } from 'k6';
// grafana sse fun fail,use phymbert/xk6-sse
// import { sse } from 'k6/x/sse';
import sse from 'k6/x/sse'
import crypto from 'k6/crypto';
import encoding from 'k6/encoding';
import { Trend, Counter, Rate } from 'k6/metrics';
import { SharedArray } from 'k6/data';
import { textSummary } from 'https://jslib.k6.io/k6-utils/1.4.0/index.js';

// run shell 
// ./k6 run sse_test.js
// ./k6 run -e SCENARIO=producer sse_test.js    
// ./k6 run -e SCENARIO=consumers sse_test.js
// --- config ---
const BASE_URL = 'http://127.0.0.1:30000';
const HMAC_SECRET = __ENV.HMAC_SECRET || "NA";
const EVENT_TYPE = 'cart_changed';
const MAX_CONSUMER_VUS = 50;
// 
const messageLatency = new Trend('message_latency_ms', true);
const receivedMessageCount = new Counter('received_message_count');
const receivedMessageSize = new Counter('received_message_size_bytes');
const sseConnectionOk = Rate('sse_connection_ok');
const producedMessageCount = new Counter('produced_message_count');
const producedMessageSize = new Counter('produced_message_size_bytes');

// 使用 SharedArray 来确保生产者和消费者使用同一组 event_id
const eventIds = new SharedArray('all-event-ids', function () {
    const ids = [];
    for (let i = 1; i <= MAX_CONSUMER_VUS; i++) {
        ids.push(`carid${String(i).padStart(6, '0')}`);
    }
    return ids;
});

let scenarios = {
    // 场景1: 消费者，订阅SSE消息
    consumers: {
        executor: 'ramping-vus',
        exec: 'consumers',
        stages: [
            { duration: '15s', target: 20 },
            // vus add count 20~50
            { duration: '1m', target: MAX_CONSUMER_VUS },
            { duration: '2m', target: MAX_CONSUMER_VUS },
            { duration: '30s', target: 0 },
        ],
    },
    // 场景2: 生产者，调用API发送消息
    producers: {
        // executor: 
        // per-vu-iterations, 每个vus执行次数
        // shared-iterations, 所有vus执行次数
        // constant-vus
        executor: 'constant-vus',
        // 指定执行的函数名
        exec: 'producers',
        // 使用2个虚拟用户持续发送消息
        vus: 2,
        // use shared-iterations then use: maxDuration and iterations
        // maxDuration: '4m',
        // iterations: 10,
        "duration": '1m15s',
        // 延迟10秒启动，让消费者先建立连接
        startTime: '45s',
    },
};
// 根据环境变量 SCENARIO 动态选择要运行的场景
if (__ENV.SCENARIO && scenarios[__ENV.SCENARIO]) {
    const scenario = scenarios[__ENV.SCENARIO];
    scenarios = {};
    scenarios[__ENV.SCENARIO] = scenario;
}

let thresholds = {

};

// 根据运行的场景动态添加特定的阈值
if (!__ENV.SCENARIO || __ENV.SCENARIO === 'consumers') {
    // 如果运行所有场景，或只运行消费者场景，则添加这些阈值
    Object.assign(thresholds, {
        'sse_connection_ok{scenario:consumers}': ['rate>0.99'],
        'message_latency_ms{scenario:consumers}': ['p(95)<1000'],
        'received_message_count{scenario:consumers}': ['count > 0'],
        'received_message_size_bytes{scenario:consumers}': ['count > 0']
    });
}
if (!__ENV.SCENARIO || __ENV.SCENARIO === 'producers') {
    // 如果运行所有场景，或只运行生产者场景，则添加这个阈值
    Object.assign(thresholds, {
        'http_req_failed{scenario:producers}': ['rate<0.01'],
        'produced_message_count{scenario:producers}': ['count > 0'],
        'produced_message_size_bytes{scenario:producers}': ['count > 0'],
        'http_req_duration{scenario:producers}': ['p(95)<500'],
    });
}

// --- 压测场景 ---
export const options = {
    scenarios: scenarios,
    thresholds: thresholds,
};

export function consumers() {
    // 确保 VU ID 不会超出预生成ID数组的范围
    if (__VU > eventIds.length) {
        return;
    }
    // 每个消费者VU从共享数组中获取一个固定的 event_id
    const eventId = eventIds[__VU - 1];

    // 1. 根据 auth_middle.rs 的逻辑生成 HMAC 签名
    const payload = `${eventId}:${EVENT_TYPE}`;
    const hmac = crypto.createHMAC('sha256', HMAC_SECRET);
    hmac.update(payload);
    const signature = hmac.digest('hex');

    // 2. 构造 SSE 连接的 URL 和 Headers
    const url = `${BASE_URL}/v1/subscribe/events/${eventId}/types/${EVENT_TYPE}`;
    const params = {
        headers: {
            'x-signature': signature,
            'User-Agent': 'k6-load-test',
        },
    };

    let responseStatus = -1;
    // 3. 发起 SSE 连接
    const res = sse.open(url, params, function (client) {
        client.on('open', function open() {
            console.log(`connected ${url}`)
        })
        client.on('event', function (event) {
            // 4. 定义事件处理器
            if (event.error) {
                console.error(`VU ${__VU}: SSE Error: ${event.error}`);
                return;
            }
            // 更新消息数量和大小的指标
            receivedMessageCount.add(1);
            // event.data 是一个字符串，其 .length 属性可以近似作为其字节大小
            receivedMessageSize.add(event.data.length);
            // sse send keep-alive message
            if (!event.data || event.data.trim() === '') {
                // console.log(`VU ${__VU}: Received an empty event, ignoring.`);
                return;
            }
            // 当收到消息时
            // console.log(`VU ${__VU}: Received event: id=${event.id}, event=${event.event}, data=${event.data}`);
            let payload;
            try {
                payload = JSON.parse(event.data);
            } catch (e) {
                console.error(`VU ${__VU}: Failed to parse event data as JSON: ${event.data}, ${url}, status code:${responseStatus}, error:${e}`);
                return;
            }

            // 增加健壮性检查：确保 payload 是一个有效的对象
            if (typeof payload !== 'object' || payload === null) {
                console.error(`VU ${__VU}: Parsed payload is not an object: ${JSON.stringify(payload)}`);
                return;
            }

            const receivedTime = new Date().getTime();
            // 检查报文中是否有 encoding 字段
            if (payload.encoding) {
                if (payload.encoding === 'base64+gzip' && payload.data) {
                    try {
                        const decodedData = encoding.b64decode(payload.data, 'std', 's');
                        const decompressedData = crypto.gzip(decodedData, 'decompress');
                        const decompressedPayloadStr = String.fromCharCode.apply(null, new Uint8Array(decompressedData));
                        payload = JSON.parse(decompressedPayloadStr);
                        latency(payload, receivedTime);
                    } catch (e) {
                        console.error(`VU ${__VU}: Failed to decompress message: ${e}`);
                        return;
                    }
                }
            } else {
                latency(payload, receivedTime);
            }

            // 可以添加检查来验证消息内容
            check(event, {
                'event data is not empty': (e) => e.data.length > 0,
            });
        })

        // error handling
        client.on('error', function (e) {
            console.log('An unexpected error occurred: ', e.error())
        })
    });

    if (!res) {
        sseConnectionOk.add(false);
        console.error(`VU ${__VU}: Failed to initiate SSE request (response is undefined).`);
        return; // Stop this iteration if the request itself failed to be created.
    }

    sseConnectionOk.add(res.status === 200);
    responseStatus = res.status;
    check(res, {
        'sse connection established': (r) => r && r.status === 200,
    });

    if (res.status !== 200) {
        console.error(`VU ${__VU}: Could not connect to SSE endpoint. Status: ${res.status}, Body: ${res.body}`);
        sleep(1);
    }

    function latency(payload, receivedTime) {
        if (payload.headers && payload.headers.send_time) {
            const sendTimeStr = payload.headers.send_time;
            // JS Date可以直接解析 "YYYY-MM-dd HH:mm:ss.sss" 格式
            const sendTime = new Date(sendTimeStr).getTime();
            const receivedTime_str = new Date(receivedTime).toISOString().replace('T', ' ').slice(0, -1);
            const sendTime_str = new Date(sendTime).toISOString().replace('T', ' ').slice(0, -1);


            if (!isNaN(sendTime)) {
                const latency = receivedTime - sendTime;
                console.log(`VU ${__VU}: Received message ${payload} with ${receivedTime_str} - ${sendTime_str} = latency: ${latency} ms`);
                // 将延迟添加到自定义指标中
                messageLatency.add(latency);
            } else {
                console.error(`VU ${__VU}: Could not parse send_time: '${sendTimeStr}'`);
            }
        }
    }
}

// 生产者函数
export function producers() {
    // 1. 从共享的 event_id 数组中随机选择一个作为目标
    const randomIndex = Math.floor(Math.random() * eventIds.length);
    const eventId = eventIds[randomIndex];
    const bodySizeKB = 10;

    // 2. 准备请求的 URL 和 Body
    const url = `${BASE_URL}/v1/sse/events`;
    const payload = JSON.stringify({
        event_id: eventId,
        body_size: bodySizeKB,
    });

    const params = {
        headers: {
            'Content-Type': 'application/json',
            'User-Agent': 'k6-producer',
        },
        //  
        tags: {
            name: 'producer-request',
        },
    };

    // 3. 发送 POST 请求
    const res = http.post(url, payload, params);

    // 4. 检查请求是否成功
    check(res, {
        'producer POST status is 200': (r) => r.status === 200,
        'producer response contains message_id': (r) => r.json('data.message_id') !== null,
    });

    producedMessageCount.add(1);
    producedMessageSize.add(bodySizeKB * 1024);
    // 5. 控制发送频率，例如每秒发送一次
    sleep(1);
}

export function handleSummary(data) {
    const producedCount = data.metrics.produced_message_count?.values?.count || 0;
    const receivedCount = data.metrics.received_message_count?.values?.count || 0;
    console.log(`handleSummary Produced ${producedCount} messages, received ${receivedCount} messages.`);
    const lostMessages = producedCount - receivedCount;
    let lossRate = 0;
    if (producedCount > 0) {
        lossRate = lostMessages / producedCount;
    }

    const lossPercentage = (lossRate * 100).toFixed(2) + "%";

    const customReport = `
    ================================================================
                          SSE Message Loss Report
    ----------------------------------------------------------------
      Messages Produced : ${producedCount}
      Messages Received : ${receivedCount}
      Messages Lost     : ${lostMessages}
      Message Loss Rate : ${lossPercentage}
    ================================================================
    `;
    console.log(`handleSummary Message Loss Rate: ${customReport}`);
    return {
        'stdout': textSummary(data, { indent: ' ', enableColors: true })
    };
}
