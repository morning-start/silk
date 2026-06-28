var myHeaders = new Headers();
myHeaders.append(
  "Authorization",
  "Bearer sk-gw-27104a64f5814e3f86faefbe82b19790",
);
myHeaders.append("User-Agent", "Apifox/1.0.0 (https://apifox.com)");
myHeaders.append("Content-Type", "application/json");
myHeaders.append("Accept", "*/*");
myHeaders.append("Host", "127.0.0.1:2013");
myHeaders.append("Connection", "keep-alive");

var raw = JSON.stringify({
  model: "agnes-2.0-flash",
  messages: [
    {
      role: "user",
      content: "你好！",
    },
  ],
});

var requestOptions = {
  method: "POST",
  headers: myHeaders,
  body: raw,
  redirect: "follow",
};

fetch("http://127.0.0.1:2013/v1/chat/completions", requestOptions)
  .then((response) => response.text())
  .then((result) => console.log(result))
  .catch((error) => console.log("error", error));
