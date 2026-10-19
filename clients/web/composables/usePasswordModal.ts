import { ref } from 'vue';

interface PasswordPromptRequest {
  actionName: string;
  resolve: (password: string | null) => void;
  errorMessage?: string;
}

const isPromptOpen = ref(false);
const activeRequest = ref<PasswordPromptRequest | null>(null);

export function usePasswordModal() {
  function promptPassword(actionName: string = 'this action', errorMessage: string = ''): Promise<string | null> {
    return new Promise((resolve) => {
      activeRequest.value = {
        actionName,
        resolve,
        errorMessage,
      };
      isPromptOpen.value = true;
    });
  }

  function submit(password: string) {
    if (activeRequest.value) {
      activeRequest.value.resolve(password);
      activeRequest.value = null;
    }
    isPromptOpen.value = false;
  }

  function cancel() {
    if (activeRequest.value) {
      activeRequest.value.resolve(null);
      activeRequest.value = null;
    }
    isPromptOpen.value = false;
  }

  function setError(msg: string) {
    if (activeRequest.value) {
      activeRequest.value.errorMessage = msg;
    }
  }

  return {
    isOpen: isPromptOpen,
    activeRequest,
    promptPassword,
    submit,
    cancel,
    setError,
  };
}
