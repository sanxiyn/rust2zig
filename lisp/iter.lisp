(defpackage #:iter
  (:use #:common-lisp)
  (:shadow #:position)
  (:export #:position #:position2))

(in-package #:iter)

(declaim (ftype (function (vector t) (or null fixnum)) position))
(defun position (l v)
  (let ((i 0))
    (declare (type fixnum i))
    (loop for e across l
          do (when (equal e v)
               (return))
             (incf i))
    (if (= i (length l))
        nil
        i)))

(declaim (ftype (function (vector t) (or null fixnum)) position2))
(defun position2 (l v)
  (loop for i of-type fixnum from 0
        for e across l
        do (when (equal e v)
             (return-from position2 i)))
  nil)

(defun test-position ()
  (let ((l #(1 2 3 4 5))
        (v 3))
    (declare (type vector l)
             (type (signed-byte 32) v))
    (assert (equal 2 (position l v)))
    (let ((v 6))
      (declare (type (signed-byte 32) v))
      (assert (equal nil (position l v))))))

(defun test-position2 ()
  (let ((l #(1 2 3 4 5))
        (v 3))
    (declare (type vector l)
             (type (signed-byte 32) v))
    (assert (equal 2 (position2 l v)))
    (let ((v 6))
      (declare (type (signed-byte 32) v))
      (assert (equal nil (position2 l v))))))

(test-position)
(test-position2)
