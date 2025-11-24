# Sse base on axum

# Base test report
#### █ THRESHOLDS 

    http_req_duration{scenario:producers}
    ✓ 'p(95)<500' p(95)=17.58ms

    http_req_failed{scenario:producers}
    ✓ 'rate<0.01' rate=0.00%

    message_latency_ms{scenario:consumers}
    ✓ 'p(95)<1000' p(95)=939.59ms

    produced_message_count{scenario:producers}
    ✓ 'count > 0' count=416

    produced_message_size_bytes{scenario:producers}
    ✓ 'count > 0' count=4259840

    received_message_count{scenario:consumers}
    ✓ 'count > 0' count=273

    received_message_size_bytes{scenario:consumers}
    ✓ 'count > 0' count=3524241

    sse_connection_ok{scenario:consumers}
    ✓ 'rate>0.99' rate=100.00%


 #### █ TOTAL RESULTS 

    checks_total.......: 1090    4.844457/s
    checks_succeeded...: 100.00% 1090 out of 1090
    checks_failed......: 0.00%   0 out of 1090

    ✓ producer POST status is 200
    ✓ producer response contains message_id
    ✓ event data is not empty
    ✓ sse connection established

    CUSTOM
    message_latency_ms.............: avg=491.84ms min=6ms    med=479ms   max=1s      p(90)=892.2ms p(95)=939.59ms
      { scenario:consumers }.......: avg=491.84ms min=6ms    med=479ms   max=1s      p(90)=892.2ms p(95)=939.59ms
    produced_message_count.........: 416     1.848894/s
      { scenario:producers }.......: 416     1.848894/s
    produced_message_size_bytes....: 4259840 18932.671615/s
      { scenario:producers }.......: 4259840 18932.671615/s
    received_message_count.........: 273     1.213336/s
      { scenario:consumers }.......: 273     1.213336/s
    received_message_size_bytes....: 3524241 15663.334197/s
      { scenario:consumers }.......: 3524241 15663.334197/s
    sse_connection_ok..............: 100.00% 1 out of 1
      { scenario:consumers }.......: 100.00% 1 out of 1
    sse_event......................: 273     1.213336/s

    HTTP
    http_req_duration..............: avg=424.4ms  min=2.62ms med=11.45ms max=2m52s   p(90)=16.38ms p(95)=17.62ms 
      { expected_response:true }...: avg=11.45ms  min=2.62ms med=11.45ms max=25.07ms p(90)=16.37ms p(95)=17.58ms 
      { scenario:producers }.......: avg=11.45ms  min=2.62ms med=11.45ms max=25.07ms p(90)=16.37ms p(95)=17.58ms 
    http_req_failed................: 0.00%   0 out of 416
      { scenario:producers }.......: 0.00%   0 out of 416
    http_reqs......................: 417     1.853338/s

    EXECUTION
    iteration_duration.............: avg=130.29µs min=250ns  med=458ns   max=2m52s   p(90)=708ns   p(95)=875ns   
    iterations.....................: 4573826 20328.168589/s
    vus............................: 1       min=1          max=52
    vus_max........................: 52      min=52         max=52

    NETWORK
    data_received..................: 3.6 MB  16 kB/s
    data_sent......................: 84 kB   372 B/s


`running (3m45.0s), 00/52 VUs, 4573826 complete and 47 interrupted iterations`
 - consumers ✓ [======================================] 01/50 VUs  3m15s
 - producers ✓ [======================================] 2 VUs      3m30s
