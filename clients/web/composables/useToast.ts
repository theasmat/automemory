import { ref } from 'vue';

export interface ToastItem {
  id: string;
  message: string;
  type: 'info' | 'success' | 'warn' | 'bad';
  duration?: number;
  undoAction?: () => void | Promise<void>;
  undoLabel?: string;
  createdAt: number;
}

const toasts = ref<ToastItem[]>([]);

export function useToast() {
  function show(toast: Omit<ToastItem, 'id' | 'createdAt'>) {
    const id = Math.random().toString(36).substring(2, 9);
    const item: ToastItem = {
      ...toast,
      id,
      createdAt: Date.now(),
      duration: toast.duration ?? (toast.type === 'bad' ? 0 : 5000),
    };

    // Queue up to 3
    if (toasts.value.length >= 3) {
      toasts.value.shift();
    }
    toasts.value.push(item);

    if (item.duration && item.duration > 0) {
      setTimeout(() => {
        dismiss(id);
      }, item.duration);
    }

    return id;
  }

  function dismiss(id: string) {
    toasts.value = toasts.value.filter((t) => t.id !== id);
  }

  function success(message: string, undoAction?: () => void | Promise<void>, undoLabel: string = 'Undo') {
    return show({
      message,
      type: 'success',
      duration: undoAction ? 8000 : 5000,
      undoAction,
      undoLabel,
    });
  }

  function error(message: string) {
    return show({
      message,
      type: 'bad',
      duration: 0, // persist until dismissed
    });
  }

  function warn(message: string) {
    return show({
      message,
      type: 'warn',
      duration: 6000,
    });
  }

  function info(message: string) {
    return show({
      message,
      type: 'info',
      duration: 4000,
    });
  }

  return {
    toasts,
    show,
    dismiss,
    success,
    error,
    warn,
    info,
  };
}
