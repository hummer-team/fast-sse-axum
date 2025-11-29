import http from 'k6/http';
import { check, sleep } from 'k6';
// grafana sse fun fail,use phymbert/xk6-sse
// import { sse } from 'k6/x/sse';
import sse from 'k6/x/sse'
import crypto from 'k6/crypto';
import encoding from 'k6/encoding';
import pako from './pako.min.js';
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
const decompressLatency = new Trend('decompress_latency_ms', true);

const eventIds = new SharedArray('all-event-ids', function () {
    const ids = [];
    for (let i = 1; i <= MAX_CONSUMER_VUS; i++) {
        ids.push(`carid${String(i).padStart(6, '0')}`);
    }
    return ids;
});

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
if (__ENV.SCENARIO && scenarios[__ENV.SCENARIO]) {
    const scenario = scenarios[__ENV.SCENARIO];
    scenarios = {};
    scenarios[__ENV.SCENARIO] = scenario;
}

let thresholds = {

};

if (!__ENV.SCENARIO || __ENV.SCENARIO === 'consumers') {
    Object.assign(thresholds, {
        'sse_connection_ok{scenario:consumers}': ['rate>0.99'],
        'message_latency_ms{scenario:consumers}': ['p(95)<1000'],
        'received_message_count{scenario:consumers}': ['count > 0'],
        'received_message_size_bytes{scenario:consumers}': ['count > 0'],
        'decompress_latency_ms{scenario:consumers}': ['p(95)<1000']
    });
}
if (!__ENV.SCENARIO || __ENV.SCENARIO === 'producers') {
    Object.assign(thresholds, {
        'http_req_failed{scenario:producers}': ['rate<0.01'],
        'produced_message_count{scenario:producers}': ['count > 0'],
        'produced_message_size_bytes{scenario:producers}': ['count > 0'],
        'http_req_duration{scenario:producers}': ['p(95)<500'],
    });
}

export const options = {
    scenarios: scenarios,
    thresholds: thresholds,
};

const numProducers = options.scenarios.producers ? options.scenarios.producers.vus : 0;

function getBeijingTimeString() {
    const now = new Date();
    const beijingOffset = 8 * 60 * 60 * 1000; // UTC+8
    const beijingTime = new Date(now.getTime() + beijingOffset);
    return beijingTime.toISOString().replace('T', ' ').slice(0, -1);
}

export function consumers() {
    if (__VU > eventIds.length) {
        return;
    }
    const eventId = eventIds[__VU - 1];

    const payload = `${eventId}:${EVENT_TYPE}`;
    const hmac = crypto.createHMAC('sha256', HMAC_SECRET);
    hmac.update(payload);
    const signature = hmac.digest('hex');

    const url = `${BASE_URL}/v1/subscribe/events/${eventId}/types/${EVENT_TYPE}`;
    const params = {
        headers: {
            'x-signature': signature,
            'User-Agent': 'k6-load-test',
        },
    };

    let responseStatus = -1;
    const res = sse.open(url, params, function (client) {
        client.on('open', function open() {
            console.log(`[${getBeijingTimeString()}] connected ${url}`)
        });

        client.on('event', function (event) {
            if (event.error) {
                console.error(`[${getBeijingTimeString()}] VU ${__VU}: SSE Error: ${event.error}`);
                return;
            }

            // sse send keep-alive message
            if (!event.data || event.data.trim() === '') {
                // console.log(`VU ${__VU}: Received an empty event, ignoring.`);
                return;
            }
            // console.log(`VU ${__VU}: Received event: id=${event.id}, event=${event.event}, data=${event.data}`);
            let payload;
            try {
                payload = JSON.parse(event.data);
            } catch (e) {
                console.error(`[${getBeijingTimeString()}] VU ${__VU}: Failed to parse event data as JSON: ${event.data}, ${url}, status code:${responseStatus}, error:${e}`);
                return;
            }

            if (typeof payload !== 'object' || payload === null) {
                console.error(`[${getBeijingTimeString()}] VU ${__VU}: Parsed payload is not an object: ${JSON.stringify(payload)}`);
                return;
            }

            receivedMessageCount.add(1);
            receivedMessageSize.add(event.data.length);
            const receivedTime = new Date().getTime();
            if (payload.encoding) {
                if (payload.encoding === 'base64+gzip' && payload.data) {
                    try {
                        const startTime = new Date().getTime();
                        const decodedData = encoding.b64decode(payload.data, 'std');
                        const decompressedData = pako.ungzip(decodedData);
                        const endTime = new Date().getTime() - startTime;
                        decompressLatency.add(endTime);
                        const decompressedPayloadStr = String.fromCharCode.apply(null, decompressedData);
                        payload = JSON.parse(decompressedPayloadStr);
                        latency(payload, receivedTime);
                    } catch (e) {
                        console.error(`[${getBeijingTimeString()}] VU ${__VU}: Failed to decompress message: ${e}`);
                        return;
                    }
                }
            } else {
                decompressLatency.add(0);
                latency(payload, receivedTime);
            }

            check(event, {
                'event data is not empty': (e) => e.data.length > 0,
            });
        });
    });
    if (!res) {
        sseConnectionOk.add(false);
        console.error(`[${getBeijingTimeString()}] VU ${__VU}: Failed to initiate SSE request (response is undefined).`);
        return;
    }

    sseConnectionOk.add(res.status === 200);
    responseStatus = res.status;
    check(res, {
        'sse connection established': (r) => r && r.status === 200,
    });

    if (res.status !== 200) {
        console.error(`[${getBeijingTimeString()}] VU ${__VU}: Could not connect to SSE endpoint. Status: ${res.status}, Body: ${res.body}`);
        sleep(1);
    }
}

function latency(payload, receivedTime) {
    if (payload.headers && payload.headers.send_time) {
        const sendTimeStr = payload.headers.send_time;
        const sendTime = new Date(sendTimeStr).getTime();
        //const beijingOffset = 8 * 60 * 60 * 1000;
        //const receivedTime_str = new Date(receivedTime + beijingOffset).toISOString().replace('T', ' ').slice(0, -1);
        //const sendTime_str = new Date(sendTime + beijingOffset).toISOString().replace('T', ' ').slice(0, -1);


        if (!isNaN(sendTime)) {
            const latency = receivedTime - sendTime;
            //console.log(`VU ${__VU}: Received message ${payload} with ${receivedTime_str} - ${sendTime_str} = latency: ${latency} ms`);
            messageLatency.add(latency);
        } else {
            console.error(`[${getBeijingTimeString()}] VU ${__VU}: Could not parse send_time: '${sendTimeStr}'`);
        }
    }
}

export function producers() {
    const producerId = __VU - 1; // 0-based ID for the current producer VU
    const globalMessageIndex = (__ITER * numProducers) + producerId;

    // Determine the target consumer by taking the modulo of the eventIds array length
    const targetIndex = globalMessageIndex % eventIds.length;
    const eventId = eventIds[targetIndex];
    const bodySizeKB = parseInt(__ENV.BODY_SIZE_KB || 10) || 10;

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
        tags: {
            name: 'producer-request',
        },
    };

    const res = http.post(url, payload, params);
    if (res.status !== 200) {
        console.error(`[${getBeijingTimeString()}] VU ${__VU} producer POST failed with status ${res.status}`);
    }

    check(res, {
        'producer POST status is 200': (r) => r.status === 200,
        'producer response contains message_id': (r) => r.json('data.message_id') !== null,
    });

    producedMessageCount.add(1);
    producedMessageSize.add(bodySizeKB * 1024);
    sleep(1);
}

export function handleSummary(data) {
    const producedCount = data.metrics.produced_message_count?.values?.count || 0;
    const receivedCount = data.metrics.received_message_count?.values?.count || 0;
    console.log(`[${getBeijingTimeString()}] handleSummary Produced ${producedCount} messages, received ${receivedCount} messages.`);
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
    console.log(`[${getBeijingTimeString()}] handleSummary Message Loss Rate: ${customReport}`);
    return {
        'stdout': textSummary(data, { indent: ' ', enableColors: true })
    };
}
