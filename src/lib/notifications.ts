import {
  getNotificationPermission as queryPermission,
  requestNotificationPermission as requestPermission,
  sendAppNotification as send,
} from "./tauri";

export type NotificationPermission = "granted" | "denied" | "prompt";

export async function getNotificationPermission(): Promise<NotificationPermission> {
  return queryPermission();
}

export async function requestNotificationPermission(): Promise<NotificationPermission> {
  try {
    return await requestPermission();
  } catch (error) {
    console.warn("请求 Windows 通知权限失败", error);
    return "denied";
  }
}

export async function sendAppNotification(title: string, body: string): Promise<boolean> {
  return send(title, body);
}
