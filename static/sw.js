self.addEventListener('push', function (event) {
  var title = 'Notifikasi';
  var body = 'Pesan baru dari Web Push.';
  if (event.data) {
    try {
      var raw = event.data.text && event.data.text();
      if (raw && raw.length > 0) {
        var data = JSON.parse(raw);
        if (data.title) title = String(data.title);
        if (data.body) body = String(data.body);
      }
    } catch (e) {
      try {
        if (event.data.json) {
          var data = event.data.json();
          if (data && data.title) title = String(data.title);
          if (data && data.body) body = String(data.body);
        }
      } catch (e2) {
        if (event.data.text) body = event.data.text() || body;
      }
    }
  }
  if (!title || title.length === 0) title = 'Notifikasi';
  if (!body || body.length === 0) body = 'Pesan baru.';

  var payload = { type: 'push-received', title: title, body: body };
  var tag = 'web-push-' + Date.now();

  try {
    var channel = new BroadcastChannel('web-push-alert');
    channel.postMessage(payload);
  } catch (e) {}

  function show() {
    return self.registration.showNotification(title, {
      body: body,
      tag: tag,
      requireInteraction: false
    });
  }

  var showPromise = show().catch(function () {
    return self.registration.showNotification(title, { body: body });
  });

  var notifyPromise = showPromise
    .then(function () {
      return self.clients.matchAll({ type: 'window', includeUncontrolled: true });
    })
    .then(function (clientList) {
      clientList.forEach(function (client) {
        if (client.postMessage) client.postMessage(payload);
      });
    })
    .catch(function () {});

  event.waitUntil(showPromise);
  event.waitUntil(notifyPromise);
});

self.addEventListener('notificationclick', function (event) {
  event.notification.close();
  event.waitUntil(
    clients.matchAll({ type: 'window', includeUncontrolled: true }).then(function (clientList) {
      if (clientList.length > 0 && clientList[0].focus) {
        return clientList[0].focus();
      }
      if (clients.openWindow) {
        return clients.openWindow('/');
      }
    })
  );
});
