// SPDX-License-Identifier: MIT
#pragma once

#include <QApplication>
#include <algorithm>
#include <QLineEdit>
#include <QPainter>
#include <QPersistentModelIndex>
#include <QPointer>
#include <QStyle>
#include <QStyleOptionViewItem>
#include <QStyledItemDelegate>
#include <QVector>

namespace mitcad {

// Trees on transparent cards need an opaque editor, independent of the
// platform style's line-edit frame and the tree's transparent Base brush.
class TreeRenameEdit : public QLineEdit {
public:
  static constexpr int kHorizontalTextMargin = 2;

  explicit TreeRenameEdit(QWidget* parent) : QLineEdit(parent) {
    setObjectName(QStringLiteral("treeRenameEditor"));
    setFrame(false);
    setTextMargins(kHorizontalTextMargin, 0, kHorizontalTextMargin, 0);
    const QPalette application = QApplication::palette();
    QPalette colors = application;
    for (QPalette::ColorGroup group : {QPalette::Active, QPalette::Inactive, QPalette::Disabled}) {
      QColor base = application.color(group, QPalette::Base);
      base.setAlpha(255);
      colors.setColor(group, QPalette::Base, base);
      // Resolve these roles explicitly: the tree's transparent Highlight
      // and a panel's accent-text role must not replace the platform
      // editor's selection pair through QWidget palette inheritance.
      for (QPalette::ColorRole role : {QPalette::Text, QPalette::Highlight, QPalette::HighlightedText}) {
        colors.setColor(group, role, application.color(group, role));
      }
    }
    setPalette(colors);
    setAttribute(Qt::WA_OpaquePaintEvent);
    setAttribute(Qt::WA_MacShowFocusRect, false);
  }

protected:
  void paintEvent(QPaintEvent* event) override {
    {
      QPainter painter(this);
      painter.fillRect(rect(), palette().brush(QPalette::Base));
    }
    QLineEdit::paintEvent(event);
  }
};

// Shared by Browser, Projects and plain panel trees. Keep the model's name
// out of the delegate's paint while its editor owns that name rectangle.
class TreeItemDelegate : public QStyledItemDelegate {
public:
  using QStyledItemDelegate::QStyledItemDelegate;

  bool editing(const QModelIndex& index) const {
    return index.isValid() && std::any_of(m_editors.cbegin(), m_editors.cend(), [&](const Editor& entry) {
      return entry.index == index && !entry.widget.isNull();
    });
  }

  // The actual name's ink area, separate from the full cell and its
  // decoration. Custom tree layouts override this along with painting.
  virtual QRect nameTextRect(const QStyleOptionViewItem& option, const QModelIndex& index) const {
    QStyleOptionViewItem cell = option;
    initStyleOption(&cell, index);
    const QStyle* style = cell.widget != nullptr ? cell.widget->style() : QApplication::style();
    const QRect text = style->subElementRect(QStyle::SE_ItemViewItemText, &cell, cell.widget);
    const int margin = style->pixelMetric(QStyle::PM_FocusFrameHMargin, nullptr, cell.widget) + 1;
    const int width = std::min(cell.fontMetrics.horizontalAdvance(cell.text), std::max(0, text.width() - 2 * margin));
    return QRect(text.left() + margin, option.rect.top() + (option.rect.height() - cell.fontMetrics.height()) / 2,
                 width, cell.fontMetrics.height());
  }

  void paint(QPainter* painter, const QStyleOptionViewItem& option, const QModelIndex& index) const override {
    QStyleOptionViewItem cell = option;
    initStyleOption(&cell, index);
    hideEditedText(cell, index);
    const QStyle* style = cell.widget != nullptr ? cell.widget->style() : QApplication::style();
    style->drawControl(QStyle::CE_ItemViewItem, &cell, painter, cell.widget);
  }

  QWidget* createEditor(QWidget* parent, const QStyleOptionViewItem& option, const QModelIndex& index) const override {
    if (index.data(Qt::EditRole).metaType().id() != QMetaType::QString) {
      return QStyledItemDelegate::createEditor(parent, option, index);
    }
    auto* editor = new TreeRenameEdit(parent);
    m_editors.append({QPersistentModelIndex(index), editor});
    QObject::connect(editor, &QObject::destroyed, this, [this, editor] { forgetEditor(editor); });
    if (parent != nullptr) {
      parent->update();
    }
    return editor;
  }

  void destroyEditor(QWidget* editor, const QModelIndex& index) const override {
    forgetEditor(editor);
    if (editor->parentWidget() != nullptr) {
      editor->parentWidget()->update();
    }
    QStyledItemDelegate::destroyEditor(editor, index);
  }

  void updateEditorGeometry(QWidget* editor, const QStyleOptionViewItem& option,
                            const QModelIndex& index) const override {
    if (!editing(index)) {
      QStyledItemDelegate::updateEditorGeometry(editor, option, index);
      return;
    }
    editor->setGeometry(editorRect(option, index));
  }

protected:
  void hideEditedText(QStyleOptionViewItem& option, const QModelIndex& index) const {
    if (editing(index)) {
      option.text.clear();
      option.features &= ~QStyleOptionViewItem::HasDisplay;
    }
  }

  virtual QRect editorRect(const QStyleOptionViewItem& option, const QModelIndex& index) const {
    QStyleOptionViewItem cell = option;
    initStyleOption(&cell, index);
    const QStyle* style = cell.widget != nullptr ? cell.widget->style() : QApplication::style();
    QRect text = style->subElementRect(QStyle::SE_ItemViewItemText, &cell, cell.widget);
    // Styles reserve their own text/focus margins inside this rectangle.
    // The frameless editor has only its explicit text margin: align its
    // text with the drawn name rather than the outer style rectangle.
    text.setLeft(std::max(option.rect.left(), nameTextRect(option, index).left() -
                                                TreeRenameEdit::kHorizontalTextMargin));
    text.setTop(option.rect.top() + 1);
    text.setBottom(option.rect.bottom() - 1);
    return text;
  }

private:
  struct Editor {
    QPersistentModelIndex index;
    QPointer<QWidget> widget;
  };

  void forgetEditor(QWidget* editor) const {
    m_editors.erase(std::remove_if(m_editors.begin(), m_editors.end(), [editor](const Editor& entry) {
      return entry.widget.isNull() || entry.widget == editor;
    }), m_editors.end());
  }

  // Persistent indexes change when rows move; keep them as values, never
  // as mutable hash keys. Usually just one editor exists in a tree.
  mutable QVector<Editor> m_editors;
};

} // namespace mitcad
