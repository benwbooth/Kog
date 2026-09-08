// Wasabi SystemObject::vcpu_integerToLongTime takes signed milliseconds.
export function integerToLongTime(value) {
  const milliseconds = Number(value) | 0;
  const hours = Math.trunc(milliseconds / 3600000);
  const minutes = Math.trunc((milliseconds % 3600000) / 60000);
  const seconds = Math.trunc((milliseconds % 60000) / 1000);
  return `${hours}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}
