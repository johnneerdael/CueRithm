// SPDX-License-Identifier: GPL-3.0-only
// Staged ASF metadata bridge; TagLib retains its own LGPL/MPL notices.
#include <asffile.h>
#include <asftag.h>
#include <asfattribute.h>
#include <asfpicture.h>
#include <memory>
#include <string>
#include <vector>
#include <stdexcept>
#include <cstdint>
struct CwAsf {std::unique_ptr<TagLib::ASF::File> file;std::string buffer,error;};
static thread_local std::string open_error;
static TagLib::String text(const unsigned char *s,size_t n){return TagLib::String(TagLib::ByteVector(reinterpret_cast<const char*>(s),static_cast<unsigned>(n)),TagLib::String::UTF8);}
static TagLib::ASF::Tag *tag(CwAsf *h){return h->file->tag();}
extern "C" {
void *cw_asf_open(const void *path,size_t len){try{
#ifdef _WIN32
 std::wstring name(static_cast<const wchar_t*>(path),len);auto file=std::make_unique<TagLib::ASF::File>(name.c_str(),false);
#else
 std::string name(static_cast<const char*>(path),len);auto file=std::make_unique<TagLib::ASF::File>(name.c_str(),false);
#endif
 if(!file->isValid()||!file->tag())throw std::runtime_error("Invalid ASF metadata");
 auto h=new CwAsf;h->file=std::move(file);return h;
}catch(const std::exception &e){open_error=e.what();return nullptr;}}
void cw_asf_free(CwAsf *h){delete h;}
const char *cw_asf_error(CwAsf *h){return h?h->error.c_str():open_error.c_str();}
int cw_asf_get(CwAsf *h,const unsigned char *key,size_t kl,size_t index,const unsigned char **out,size_t *len){try{
 auto k=text(key,kl);TagLib::String value;bool basic=true;
 if(k=="Title")value=tag(h)->title();else if(k=="Author")value=tag(h)->artist();else if(k=="Description")value=tag(h)->comment();else basic=false;
 if(basic){if(index||value.isEmpty())return 0;}
 else {auto values=tag(h)->attribute(k);if(index>=values.size())return 0;const auto &a=values[static_cast<unsigned>(index)];
 switch(a.type()){
 case TagLib::ASF::Attribute::UnicodeType:value=a.toString();break;
 case TagLib::ASF::Attribute::DWordType:value=TagLib::String(std::to_string(a.toUInt()),TagLib::String::UTF8);break;
 case TagLib::ASF::Attribute::QWordType:value=TagLib::String(std::to_string(a.toULongLong()),TagLib::String::UTF8);break;
 case TagLib::ASF::Attribute::WordType:value=TagLib::String(std::to_string(a.toUShort()),TagLib::String::UTF8);break;
 case TagLib::ASF::Attribute::BoolType:value=a.toBool()?"1":"0";break;
 default:return 0;}}
 h->buffer=value.to8Bit(true);*out=reinterpret_cast<const unsigned char*>(h->buffer.data());*len=h->buffer.size();return 1;
}catch(const std::exception &e){h->error=e.what();return -1;}}
int cw_asf_keys(CwAsf *h,size_t index,const unsigned char **out,size_t *len){try{
 std::vector<TagLib::String> keys={"Title","Author","Description"};const auto &attrs=static_cast<const TagLib::ASF::Tag*>(tag(h))->attributeListMap();for(auto it=attrs.begin();it!=attrs.end();++it)keys.push_back(it->first);
 if(index>=keys.size())return 0;h->buffer=keys[index].to8Bit(true);*out=reinterpret_cast<const unsigned char*>(h->buffer.data());*len=h->buffer.size();return 1;
}catch(const std::exception &e){h->error=e.what();return -1;}}
int cw_asf_clear(CwAsf *h,const unsigned char *key,size_t kl){try{auto k=text(key,kl);if(k=="Title")tag(h)->setTitle("");else if(k=="Author")tag(h)->setArtist("");else if(k=="Description")tag(h)->setComment("");else tag(h)->removeItem(k);return 1;}catch(const std::exception &e){h->error=e.what();return 0;}}
int cw_asf_add(CwAsf *h,const unsigned char *key,size_t kl,const unsigned char *value,size_t vl){try{
 auto k=text(key,kl),v=text(value,vl);
 if(k=="Title")tag(h)->setTitle(v);else if(k=="Author")tag(h)->setArtist(v);else if(k=="Description")tag(h)->setComment(v);else tag(h)->addAttribute(k,TagLib::ASF::Attribute(v));return 1;
}catch(const std::exception &e){h->error=e.what();return 0;}}
int cw_asf_picture(CwAsf *h,size_t index,unsigned *kind,const unsigned char **bytes,size_t *len,const unsigned char **mime,size_t *ml){try{
 auto attrs=tag(h)->attribute("WM/Picture");if(index>=attrs.size())return 0;auto pic=attrs[static_cast<unsigned>(index)].toPicture();if(!pic.isValid())return 2;
 // Buffers must remain valid until the next call; use two thread-local copies.
 static thread_local std::string image,mime_value;auto data=pic.picture();image.assign(data.data(),data.size());mime_value=pic.mimeType().to8Bit(true);
 *kind=static_cast<unsigned>(pic.type());*bytes=reinterpret_cast<const unsigned char*>(image.data());*len=image.size();*mime=reinterpret_cast<const unsigned char*>(mime_value.data());*ml=mime_value.size();return 1;
}catch(const std::exception &e){h->error=e.what();return -1;}}
int cw_asf_remove_picture(CwAsf *h,unsigned kind){try{auto attrs=tag(h)->attribute("WM/Picture");TagLib::ASF::AttributeList keep;for(const auto &a:attrs){auto pic=a.toPicture();if(!pic.isValid()||static_cast<unsigned>(pic.type())!=kind)keep.append(a);}tag(h)->setAttribute("WM/Picture",keep);return 1;}catch(const std::exception &e){h->error=e.what();return 0;}}
int cw_asf_set_picture(CwAsf *h,unsigned kind,const unsigned char *mime,size_t ml,const unsigned char *description,size_t dl,const unsigned char *data,size_t len){try{TagLib::ASF::Picture p;p.setType(static_cast<TagLib::ASF::Picture::Type>(kind));p.setMimeType(text(mime,ml));p.setDescription(text(description,dl));p.setPicture(TagLib::ByteVector(reinterpret_cast<const char*>(data),static_cast<unsigned>(len)));if(!p.isValid())throw std::runtime_error("Invalid ASF picture");tag(h)->addAttribute("WM/Picture",TagLib::ASF::Attribute(p));return 1;}catch(const std::exception &e){h->error=e.what();return 0;}}
int cw_asf_save(CwAsf *h){try{if(!h->error.empty())return 0;if(!h->file->save())throw std::runtime_error("Save staged ASF metadata failed");return 1;}catch(const std::exception &e){h->error=e.what();return 0;}}
}
