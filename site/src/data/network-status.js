class NetworkStatus {
  get offline() {
    return typeof navigator !== "undefined" && "onLine" in navigator && !navigator.onLine;
  }

  onChange(handler) {
    window.addEventListener("online", handler);
    window.addEventListener("offline", handler);
    return () => {
      window.removeEventListener("online", handler);
      window.removeEventListener("offline", handler);
    };
  }
}

const networkStatus = new NetworkStatus();

export { networkStatus };
