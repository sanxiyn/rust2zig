(defpackage #:hash
  (:use #:common-lisp)
  (:import-from #+sbcl #:sb-ext #+ecl #:ext #:string-to-octets)
  (:export #:fnv1a-hash-32 #:fnv1a-hash-str-32))

(in-package #:hash)

(defconstant +fnv-offset-basis-32+ 2166136261)
(defconstant +fnv-prime-32+ 16777619)

(declaim (ftype (function ((vector (unsigned-byte 8)) (or null fixnum)) (unsigned-byte 32)) fnv1a-hash-32))
(defun fnv1a-hash-32 (bytes limit)
  (let ((prime +fnv-prime-32+)
        (hash +fnv-offset-basis-32+)
        (i 0)
        (len (let ((v limit))
               (cond ((and v (< 0 v) (< v (length bytes))) v)
                     (t (length bytes))))))
    (declare (type (unsigned-byte 32) prime hash)
             (type fixnum i len))
    (loop while (< i len)
          do (setf hash (logxor hash (aref bytes i)))
             (setf hash (ldb (byte 32 0) (* hash prime)))
             (incf i))
    hash))

(declaim (ftype (function (string) (unsigned-byte 32)) fnv1a-hash-str-32))
(defun fnv1a-hash-str-32 (input)
  (fnv1a-hash-32 (string-to-octets input :external-format :utf-8) nil))

(defconstant +foobar+ "foobar")
(defconstant +foobar-hash-32+ 3214735720)

(defun test-32 ()
  (let ((hashed (fnv1a-hash-str-32 +foobar+)))
    (declare (type (unsigned-byte 32) hashed))
    (assert (= +foobar-hash-32+ hashed))))

(test-32)
